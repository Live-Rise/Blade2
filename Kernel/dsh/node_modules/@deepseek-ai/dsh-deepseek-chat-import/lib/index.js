import { createHash, randomUUID } from "node:crypto";
import Schema from "@deepseek-ai/schemastery";
import { Remote, RemoteError, TypertRemoteService, principalOf } from "@deepseek-ai/dsh-typert-protocol";
import { stat } from "node:fs/promises";
import { extname } from "node:path";
import { SESSION_FORMAT_VERSION, Session, SessionId, SessionLogOffset, SessionSeq } from "@deepseek-ai/dsh-session";
import { brandString } from "@deepseek-ai/dsh-brand";
import { MessageId, ToolCallId } from "@deepseek-ai/dsh-llm";
import { normalizeSessionTitle } from "@deepseek-ai/dsh-session-title";
import { fetchMetaFromValue, formatFetchOutput, formatSearchOutput, searchMetaFromValue } from "@deepseek-ai/dsh-tool-web";
import { z } from "zod";
import { setImmediate } from "node:timers/promises";
import { Unzip, UnzipInflate, unzip } from "fflate";
//#region lib/types/format.js
/**
* JSON validation for the DeepSeek chat export (`export_format_version: 3`).
*
* Only fields the importer reads are validated. Fragment types the importer
* does not map are accepted unchecked so later exporter additions do not
* reject otherwise importable conversations.
*/
/** `manifest.json`, the first archive entry. */
const exportManifest = z.object({ export_format_version: z.number() });
/** One export timestamp, converted to epoch milliseconds so ordering never re-parses it. */
const exportTime = z.string().transform((value, ctx) => {
	const time = parseExportTime(value);
	if (time === void 0) {
		ctx.addIssue({
			code: "custom",
			message: "invalid timestamp"
		});
		return z.NEVER;
	}
	return time;
});
/**
* `chat_session_index.json`: one row per conversation, carrying the update
* order the import selects by. The index's title, pinned flag, and message
* count are not read and stay unvalidated.
*/
const exportSessionIndex = z.array(z.object({
	id: z.string().min(1),
	inserted_at: exportTime,
	updated_at: exportTime
}));
const fragmentEnvelope = z.looseObject({ type: z.string() });
const exportMessage = z.object({
	id: z.number().int().nonnegative(),
	parent_id: z.number().int().nonnegative().nullable(),
	role: z.enum(["USER", "ASSISTANT"]),
	status: z.string(),
	inserted_at: z.string(),
	fragments: z.array(fragmentEnvelope)
});
/**
* `chat_sessions/{id}.json`: one conversation and its complete message tree.
* `current_message_id` may be absent or null; branch selection then falls back
* to the greatest message id.
*/
const exportSessionFile = z.object({
	id: z.string().min(1),
	title: z.string().nullable(),
	current_message_id: z.number().int().nonnegative().nullable().optional(),
	inserted_at: z.string(),
	messages: z.array(exportMessage)
});
const optionalText = z.string().nullable().optional();
const optionalStage = z.number().int().nullable().optional();
const searchResult = z.object({
	url: z.string(),
	title: optionalText,
	snippet: optionalText,
	published_at: optionalText
});
const readLinkResult = z.object({
	url: optionalText,
	title: optionalText
});
const fileEntry = z.object({
	id: z.string(),
	file_name: z.string(),
	/** Declared byte size, shown on the synthetic file card; an archive without it reports 0 bytes. */
	file_size: z.number().int().nonnegative().optional().default(0),
	is_image: z.boolean(),
	export_path: z.string().nullable()
});
const knownFragment = z.discriminatedUnion("type", [
	z.object({
		type: z.literal("REQUEST"),
		content: z.string()
	}),
	z.object({
		type: z.literal("RESPONSE"),
		content: z.string()
	}),
	z.object({
		type: z.literal("THINK"),
		content: z.string(),
		elapsed_secs: z.number().nonnegative().nullable().optional()
	}),
	z.object({
		type: z.literal("TEMPLATE_RESPONSE"),
		content: z.string()
	}),
	z.object({
		type: z.literal("TOOL_SEARCH"),
		queries: z.array(z.string()),
		results: z.array(searchResult),
		status: z.string().optional(),
		stage_id: optionalStage
	}),
	z.object({
		type: z.literal("TOOL_OPEN"),
		status: z.string(),
		url: z.string().nullable(),
		title: optionalText,
		stage_id: optionalStage
	}),
	z.object({
		type: z.literal("TOOL_FIND"),
		status: z.string(),
		pattern: z.string().optional(),
		stage_id: optionalStage
	}),
	z.object({
		type: z.literal("READ_LINK"),
		results: z.array(readLinkResult)
	}),
	z.object({
		type: z.literal("TOOL_IMAGE_SEARCH"),
		status: z.string(),
		results: z.array(z.unknown()),
		stage_id: optionalStage
	}),
	z.object({
		type: z.literal("PARTIAL_FILE_READ"),
		ratio: z.number()
	}),
	z.object({
		type: z.literal("TIP"),
		content: z.string()
	}),
	z.object({
		type: z.literal("FILE"),
		files: z.array(fileEntry)
	})
]);
const KNOWN_FRAGMENT_TYPES = new Set(knownFragment.options.map((option) => option.shape.type.value));
/**
* Validate one fragment whose type the importer maps.
* @param fragment - fragment envelope from a validated message.
* @returns the typed fragment, or undefined for a type the importer ignores.
* @throws {z.ZodError} when a mapped fragment type lacks a required field.
*/
function parseKnownFragment(fragment) {
	return KNOWN_FRAGMENT_TYPES.has(fragment.type) ? knownFragment.parse(fragment) : void 0;
}
/**
* Parse an export timestamp: an RFC 3339 UTC string with 0, 3, 6, or 9 fractional digits.
* @param value - timestamp text from the export.
* @returns epoch milliseconds, or undefined when the text is not a valid timestamp.
*/
function parseExportTime(value) {
	const time = Date.parse(value);
	return Number.isSafeInteger(time) && time >= 0 ? time : void 0;
}
//#endregion
//#region lib/types/conversation.js
/**
* Conversion of one exported DeepSeek conversation tree into the seed log of
* one DSH Session.
*
* The message named by `current_message_id` selects the imported branch, with
* the greatest message id as the fallback. Each USER message opens a turn; a
* finished ASSISTANT reply becomes one or more steps whose web searches and
* opened pages replay as `web_search` and `web_fetch` calls. A question whose
* reply was withdrawn by content filtering is left out with its reply. Other
* unfinished, templated, empty, or missing replies end their turn with an
* error and never enter model history.
*/
/** Provider and model recorded on every imported assistant message. */
const IMPORTED_MODEL_SOURCE = {
	provider: "deepseek-chat-export",
	model: "deepseek-chat-export"
};
/** Character cap passed to the `web_fetch` formatter; imported page bodies are short notices. */
const FETCH_OUTPUT_CHARS = 4e3;
/** Prefix hashed with the export file id into the attachment id of a synthetic file reference. */
const SYNTHETIC_FILE_ID_PREFIX = "deepseek-chat-export:";
/** Error codes of the export statuses that end a reply unfinished; any other status uses `DEEPSEEK_EXPORT_UNFINISHED_REPLY`. */
const UNFINISHED_STATUS_CODES = new Map([
	["FAILED", "DEEPSEEK_EXPORT_FAILED"],
	["TIMEOUT", "DEEPSEEK_EXPORT_TIMEOUT"],
	["CONTEXT_LENGTH_EXCEEDED", "DEEPSEEK_EXPORT_CONTEXT_LENGTH_EXCEEDED"],
	["INCOMPLETE", "DEEPSEEK_EXPORT_INCOMPLETE"],
	["WIP", "DEEPSEEK_EXPORT_WIP"]
]);
/** A conversation file that cannot be imported; the message names the defect. */
var ExportConversationError = class extends Error {
	name = "ExportConversationError";
};
/**
* Select the imported branch of one conversation tree: the path from the root
* to the message named by `current_message_id`, or to the message with the
* greatest id when that field is absent, null, or names no message. A child
* always has a greater id than its parent, so the fallback is a leaf.
* @param file - one validated conversation file.
* @returns messages from root to the selected message.
* @throws {ExportConversationError} for an empty tree, duplicate ids, a missing parent, or a cycle.
*/
function selectBranch(file) {
	const byId = /* @__PURE__ */ new Map();
	let newest;
	for (const message of file.messages) {
		if (byId.has(message.id)) throw new ExportConversationError(`duplicate message id ${message.id}`);
		byId.set(message.id, message);
		if (newest === void 0 || message.id > newest.id) newest = message;
	}
	if (newest === void 0) throw new ExportConversationError("conversation has no messages");
	const current = file.current_message_id;
	const selected = current === null || current === void 0 ? void 0 : byId.get(current);
	const chain = [];
	for (let message = selected ?? newest;;) {
		if (chain.length === byId.size) throw new ExportConversationError(`message ${message.id} is part of a parent cycle`);
		chain.push(message);
		if (message.parent_id === null) return chain.reverse();
		const parent = byId.get(message.parent_id);
		if (parent === void 0) throw new ExportConversationError(`message parent ${message.parent_id} does not exist`);
		message = parent;
	}
}
/**
* Read the time of a message as epoch milliseconds.
* @param message - one export message.
* @returns the parsed `inserted_at`.
* @throws {ExportConversationError} when the timestamp is invalid.
*/
function messageTime(message) {
	const time = parseExportTime(message.inserted_at);
	if (time === void 0) throw new ExportConversationError(`message ${message.id} has an invalid inserted_at`);
	return time;
}
/**
* Convert one conversation into the seed log of a DSH Session.
* @param file - one validated conversation file.
* @param options - id source and saved images.
* @returns the title, creation time, and seed events.
* @throws {ExportConversationError} when the tree, a timestamp, or a mapped fragment is invalid, or no turn remains.
*/
function convertConversation(file, options) {
	const chain = selectBranch(file);
	const log = new LogBuilder(options);
	const normalized = file.title === null ? "" : normalizeSessionTitle(file.title, options.maxTitleBytes);
	const title = normalized.length === 0 ? null : normalized;
	let turn = 0;
	let withdrawn = 0;
	for (let index = 0; index < chain.length; index++) {
		const user = chain[index];
		if (user.role !== "USER") continue;
		const next = chain[index + 1];
		const reply = next?.role === "ASSISTANT" ? next : void 0;
		if (reply !== void 0) index++;
		if (reply?.status === "CONTENT_FILTER") {
			withdrawn++;
			continue;
		}
		turn++;
		log.turn(turn, user, reply);
		if (turn === 1 && title !== null) log.add({
			type: "session/title",
			data: {
				title,
				messageSeqs: [],
				source: { kind: "user" }
			}
		});
	}
	if (turn === 0) throw new ExportConversationError(withdrawn === 0 ? "conversation has no user message" : "every question of the conversation was withdrawn by content filtering");
	return {
		title,
		createdAt: log.events[0].time,
		events: log.events
	};
}
/** Accumulates seed events with contiguous seqs and non-decreasing times. */
var LogBuilder = class {
	options;
	events = [];
	time = 0;
	constructor(options) {
		this.options = options;
	}
	add(event) {
		const seq = SessionSeq(this.events.length);
		this.events.push({
			...event,
			seq,
			time: this.time
		});
		return seq;
	}
	turn(turn, user, reply) {
		this.at(messageTime(user));
		this.add({
			type: "turn/start",
			data: { turn }
		});
		let step = 1;
		this.add({
			type: "step/start",
			data: {
				turn,
				step
			}
		});
		if (turn === 1) {
			const head = {
				id: MessageId(this.id()),
				role: "system",
				content: [],
				source: { kind: "system-prompt" }
			};
			this.add({
				type: "system/message",
				data: {
					turn,
					step,
					message: head
				},
				surfaceOp: "append"
			});
		}
		this.add({
			type: "user/message",
			data: this.userMessage(user),
			surfaceOp: "append"
		});
		if (reply === void 0) {
			this.closeTurn(turn, step, failure$1("DEEPSEEK_EXPORT_NO_REPLY", "The export contains no reply to this message."));
			return;
		}
		this.at(messageTime(reply));
		const { steps, template } = this.replySteps(reply);
		const excluded = excludedReply(reply, template, steps.length);
		if (excluded !== void 0) {
			this.closeTurn(turn, step, {
				kind: "error",
				error: excluded
			});
			return;
		}
		for (const [position, { content, calls, elapsedMs }] of steps.entries()) {
			if (position > 0) {
				step++;
				this.add({
					type: "step/start",
					data: {
						turn,
						step
					}
				});
			}
			this.time += elapsedMs;
			this.add({
				type: "assistant/message",
				data: {
					turn,
					step,
					message: assistantMessage(this.id(), [...content, ...calls.map((call) => call.block)]),
					stream: []
				},
				surfaceOp: "append"
			});
			const callSeqs = calls.map(({ block }) => this.add({
				type: "tool/call",
				data: {
					turn,
					step,
					callId: block.id,
					name: block.name,
					arguments: block.arguments
				}
			}));
			for (const [callIndex, call] of calls.entries()) this.add({
				type: "tool/result",
				data: {
					turn,
					step,
					message: toolResult(this.id(), call),
					...call.meta === void 0 ? {} : { meta: call.meta }
				},
				surfaceOp: "append",
				sourceEventSeqs: [callSeqs[callIndex]]
			});
			if (position < steps.length - 1) this.add({
				type: "step/end",
				data: {
					turn,
					step
				}
			});
		}
		this.closeTurn(turn, step, { kind: "completed" });
	}
	closeTurn(turn, step, reason) {
		this.add({
			type: "step/end",
			data: {
				turn,
				step
			}
		});
		this.add({
			type: "turn/end",
			data: {
				turn,
				reason
			}
		});
	}
	at(time) {
		this.time = Math.max(this.time, time);
	}
	id() {
		return this.options.createId();
	}
	userMessage(message) {
		const attachments = [];
		const lines = [];
		for (const fragment of fragmentsOf(message)) if (fragment.type === "REQUEST") lines.push(fragment.content);
		else if (fragment.type === "FILE") for (const file of fragment.files) {
			const image = file.is_image ? this.options.images?.get(file.id) : void 0;
			attachments.push(image === void 0 ? {
				type: "file",
				attachment: syntheticFileRef(file)
			} : {
				type: "image",
				attachment: image
			});
		}
		const text = lines.join("\n");
		const content = text.trim().length > 0 ? [...attachments, {
			type: "text",
			text
		}] : attachments.length > 0 ? attachments : [{
			type: "text",
			text: ""
		}];
		return {
			id: MessageId(this.id()),
			role: "user",
			content,
			source: { kind: "user" }
		};
	}
	/**
	* Split one reply into steps. A tool fragment that carries a `stage_id`
	* starts a new step when it differs from the stage of the open step;
	* a step opened by text adopts the first stage it sees. Text after a tool
	* call starts a new step. Also reports the first server template.
	*/
	replySteps(reply) {
		const steps = [];
		let template;
		let current = emptyStep();
		const pushStep = () => {
			if (current.content.length === 0 && current.calls.length === 0) return;
			steps.push(current);
			current = emptyStep();
		};
		const appendContent = (block) => {
			if (current.calls.length > 0) pushStep();
			current.content.push(block);
		};
		const appendCall = (call, stage) => {
			if (stage !== null) {
				if (current.stage !== null && stage !== current.stage) pushStep();
				current.stage = stage;
			}
			current.calls.push(call);
		};
		for (const fragment of fragmentsOf(reply)) switch (fragment.type) {
			case "THINK":
				if (fragment.content.length > 0) appendContent({
					type: "reasoning",
					text: fragment.content
				});
				current.elapsedMs += Math.round((fragment.elapsed_secs ?? 0) * 1e3);
				break;
			case "RESPONSE":
				if (fragment.content.length > 0) appendContent({
					type: "text",
					text: fragment.content
				});
				break;
			case "TOOL_SEARCH": {
				if (fragment.status === "WIP") break;
				const call = searchCall(this.id(), fragment);
				if (call !== void 0) appendCall(call, fragment.stage_id ?? null);
				break;
			}
			case "TOOL_OPEN": {
				const url = nonEmpty(fragment.url);
				if (fragment.status === "WIP" || url === void 0) break;
				appendCall(openCall(this.id(), fragment, url), fragment.stage_id ?? null);
				break;
			}
			case "READ_LINK": {
				const text = linkLines(fragment.results);
				if (text !== void 0) appendContent({
					type: "text",
					text
				});
				break;
			}
			case "TEMPLATE_RESPONSE":
				template ??= fragment.content;
				break;
			case "TOOL_FIND":
			case "TOOL_IMAGE_SEARCH":
			case "PARTIAL_FILE_READ":
			case "TIP":
			case "REQUEST":
			case "FILE": break;
		}
		pushStep();
		return {
			steps,
			template
		};
	}
};
function fragmentsOf(message) {
	const fragments = [];
	for (const [index, envelope] of message.fragments.entries()) {
		let fragment;
		try {
			fragment = parseKnownFragment(envelope);
		} catch (error) {
			throw new ExportConversationError(`message ${message.id} fragment ${index} (${envelope.type}) is malformed`, { cause: error });
		}
		if (fragment !== void 0) fragments.push(fragment);
	}
	return fragments;
}
/**
* The failure that replaces a reply excluded from model history, or undefined
* for an importable reply. A server template outranks the status, which
* outranks an empty reply. Error codes are fixed; only the message names an
* unrecognized status.
*/
function excludedReply(reply, template, stepCount) {
	if (template !== void 0) return {
		code: "DEEPSEEK_EXPORT_TEMPLATE_RESPONSE",
		message: template
	};
	if (reply.status !== "FINISHED") return {
		code: UNFINISHED_STATUS_CODES.get(reply.status) ?? "DEEPSEEK_EXPORT_UNFINISHED_REPLY",
		message: `The original reply ended with status ${JSON.stringify(reply.status)} and is excluded from the model context.`
	};
	return stepCount === 0 ? {
		code: "DEEPSEEK_EXPORT_EMPTY_REPLY",
		message: "The original reply has no importable content."
	} : void 0;
}
function failure$1(code, message) {
	return {
		kind: "error",
		error: {
			code,
			message
		}
	};
}
/**
* The synthetic reference of one exported file whose bytes are not stored: a
* non-image file, whose body the export never carries, or an image that was
* not stored. The id is deterministic in the export file id, so re-importing
* the same export yields the same reference; no attachment object exists for
* it, so the model's handle names a path that holds no bytes; a name the store
* would rewrite makes the reference invalid instead, and that handle carries no
* path at all.
*/
function syntheticFileRef(file) {
	return {
		attachmentId: brandString(`sha256:${createHash("sha256").update(`${SYNTHETIC_FILE_ID_PREFIX}${file.id}`).digest("hex")}`),
		name: fileDisplayName(file.file_name),
		bytes: file.file_size
	};
}
/** The leaf of an exported file name without control characters; `file` when nothing remains. */
function fileDisplayName(name) {
	const clean = name.slice(Math.max(name.lastIndexOf("/"), name.lastIndexOf("\\")) + 1).replace(/[\u0000-\u001f\u007f]/gu, "").trim();
	return clean.length === 0 ? "file" : clean;
}
function emptyStep() {
	return {
		content: [],
		calls: [],
		elapsedMs: 0,
		stage: null
	};
}
/**
* The Markdown link lines of one `READ_LINK` record: `[title](url)` per
* result, the URL as its own text when the title is missing, the title alone
* without a URL; a result with neither is skipped.
* @param results - the record's results.
* @returns the joined lines, or undefined when no result names anything.
*/
function linkLines(results) {
	const lines = [];
	for (const result of results) {
		const url = nonEmpty(result.url);
		const title = nonEmpty(result.title);
		if (url !== void 0) lines.push(`[${title ?? url}](${url})`);
		else if (title !== void 0) lines.push(title);
	}
	return lines.length === 0 ? void 0 : lines.join("\n");
}
function assistantMessage(id, content) {
	return {
		id: MessageId(id),
		role: "assistant",
		content,
		source: {
			kind: "model",
			...IMPORTED_MODEL_SOURCE
		}
	};
}
function toolResult(id, call) {
	return {
		id: MessageId(id),
		role: "tool",
		content: call.result,
		isError: call.isError,
		source: {
			kind: "tool",
			callId: call.block.id
		},
		toolCallId: call.block.id
	};
}
function searchCall(id, fragment) {
	const queries = fragment.queries.filter((query) => query.trim().length > 0);
	const block = {
		type: "tool-call",
		id: ToolCallId(id),
		name: "web_search",
		arguments: JSON.stringify({ queries })
	};
	if (fragment.status === "FAILED") return {
		block,
		result: [{
			type: "text",
			text: "Error: the export records this search as failed."
		}],
		isError: true
	};
	if (queries.length === 0) return void 0;
	const result = {
		sources: fragment.results.map((source) => {
			const title = nonEmpty(source.title);
			const snippet = nonEmpty(source.snippet);
			const publishedAt = nonEmpty(source.published_at);
			return {
				url: source.url,
				...title === void 0 ? {} : { title },
				...snippet === void 0 ? {} : { snippet },
				...publishedAt === void 0 ? {} : { publishedAt }
			};
		}),
		truncated: false
	};
	return {
		block,
		result: [{
			type: "text",
			text: formatSearchOutput(result)
		}],
		isError: false,
		meta: searchMetaFromValue(result)
	};
}
function openCall(id, fragment, url) {
	const block = {
		type: "tool-call",
		id: ToolCallId(id),
		name: "web_fetch",
		arguments: JSON.stringify({ url })
	};
	if (fragment.status !== "FINISHED") return {
		block,
		result: [{
			type: "text",
			text: "Error: the page could not be opened."
		}],
		isError: true
	};
	const title = nonEmpty(fragment.title);
	const notice = "The page content is not included in the DeepSeek chat export.";
	const value = {
		url,
		statusCode: 200,
		truncated: false,
		body: {
			kind: "text",
			content: title === void 0 ? notice : `${title}\n\n${notice}`
		}
	};
	return {
		block,
		result: [{
			type: "text",
			text: formatFetchOutput(value, FETCH_OUTPUT_CHARS)
		}],
		isError: false,
		meta: fetchMetaFromValue(value, FETCH_OUTPUT_CHARS)
	};
}
function nonEmpty(value) {
	return value === null || value === void 0 || value.trim().length === 0 ? void 0 : value;
}
//#endregion
//#region lib/types/archive.js
/**
* In-memory view of one DeepSeek chat export ZIP archive. Opening reads the
* central directory; a streaming pass inflates only the entries it is asked
* for, and a by-name read serves a targeted fetch of one entry.
*/
/** The upload cannot be imported as a whole; `reason` selects the user-facing explanation. */
var ExportSourceError = class extends Error {
	reason;
	name = "ExportSourceError";
	/**
	* @param reason - failure category shown to the user.
	* @param message - English diagnostic.
	* @param options - underlying cause.
	*/
	constructor(reason, message, options) {
		super(message, options);
		this.reason = reason;
	}
};
/** One entry cannot be read: it exceeds the per-entry byte limit, or its data is damaged. */
var ExportEntryError = class extends Error {
	oversized;
	name = "ExportEntryError";
	/**
	* @param message - English diagnostic naming the entry.
	* @param oversized - true when the entry declares or stores more bytes than the per-entry limit, false when it is damaged.
	* @param options - underlying cause.
	*/
	constructor(message, oversized, options) {
		super(message, options);
		this.oversized = oversized;
	}
};
/** Bytes consumed from the archive before the pass yields to the event loop. */
const SLICE_BYTES = 64 * 1024;
/**
* Open an export archive held in memory. Opening reads only the ZIP central
* directory; entries are inflated when a pass or a read asks for them.
* @param archive - complete ZIP bytes.
* @param maxEntryBytes - largest declared uncompressed size of one entry.
* @param signal - cancellation.
* @returns a reader over the archive.
* @throws {ExportSourceError} `unreadable-archive` when the bytes are not a readable ZIP archive
* or carry more than one entry with the same name.
*/
async function openExportArchive(archive, maxEntryBytes, signal) {
	const names = [];
	const directory = /* @__PURE__ */ new Map();
	try {
		await unzipAsync(archive, (file) => {
			names.push(file.name);
			directory.set(file.name, {
				name: file.name,
				size: file.size,
				originalSize: file.originalSize,
				compression: file.compression
			});
			return false;
		}, signal);
	} catch (error) {
		signal.throwIfAborted();
		throw new ExportSourceError("unreadable-archive", "the uploaded file is not a readable ZIP archive", { cause: error });
	}
	const unique = /* @__PURE__ */ new Set();
	for (const name of names) {
		if (unique.has(name)) throw new ExportSourceError("unreadable-archive", `the archive has more than one entry named ${name}`);
		unique.add(name);
	}
	return {
		stream: (handler, streamSignal) => streamEntries(archive, directory, maxEntryBytes, handler, streamSignal),
		read: async (entry, readSignal) => {
			let rejected;
			let files;
			try {
				files = await unzipAsync(archive, (file) => {
					if (file.name !== entry) return false;
					rejected = entrySizeError(file, maxEntryBytes);
					return rejected === void 0;
				}, readSignal);
			} catch (error) {
				readSignal.throwIfAborted();
				throw new ExportEntryError(`${entry} cannot be decompressed`, false, { cause: error });
			}
			if (rejected !== void 0) throw rejected;
			return files[entry];
		}
	};
}
/**
* One streaming pass over the archive bytes, in entry order. The central
* directory supplies each entry's declared sizes before its data is inflated;
* an accepted entry's chunks are accumulated and delivered whole.
*/
async function streamEntries(archive, directory, maxEntryBytes, handler, signal) {
	const finished = [];
	const finish = (entry) => {
		const failure = entry.error ?? sizeFailure(entry, maxEntryBytes);
		finished.push(failure === void 0 ? {
			info: entry.info,
			data: concat(entry.chunks, entry.received)
		} : {
			info: entry.info,
			error: failure
		});
	};
	const inflate = (file, info) => {
		const entry = {
			info,
			chunks: [],
			received: 0,
			error: void 0
		};
		file.ondata = (dataError, chunk, final) => {
			if (dataError !== null) entry.error ??= new ExportEntryError(`${info.name} cannot be decompressed`, false, { cause: dataError });
			else if (chunk.length > 0) {
				entry.received += chunk.length;
				if (entry.received <= maxEntryBytes) entry.chunks.push(chunk);
			}
			if (final) finish(entry);
		};
		file.start();
	};
	const reader = new Unzip((file) => {
		const info = directory.get(file.name);
		if (info === void 0) throw new ExportSourceError("unreadable-archive", `the archive contains ${file.name}, which its directory does not list`);
		const error = entrySizeError(info, maxEntryBytes);
		if (error !== void 0) {
			handler.reject(info, error);
			return;
		}
		if (handler.select(info)) inflate(file, info);
	});
	reader.register(UnzipInflate);
	const deliver = async () => {
		for (const item of finished.splice(0)) if ("error" in item) handler.reject(item.info, item.error);
		else await handler.entry(item.info, item.data);
	};
	signal.throwIfAborted();
	for (let offset = 0; offset < archive.length; offset += SLICE_BYTES) {
		const end = Math.min(archive.length, offset + SLICE_BYTES);
		try {
			reader.push(archive.subarray(offset, end), end === archive.length);
		} catch (error) {
			signal.throwIfAborted();
			if (error instanceof ExportSourceError) throw error;
			throw new ExportSourceError("unreadable-archive", "the uploaded file is not a readable ZIP archive", { cause: error });
		}
		await deliver();
		signal.throwIfAborted();
		await setImmediate();
	}
	await deliver();
}
/** Join accumulated chunks into one buffer of `size` bytes. */
function concat(chunks, size) {
	const data = new Uint8Array(size);
	let offset = 0;
	for (const chunk of chunks) {
		data.set(chunk, offset);
		offset += chunk.length;
	}
	return data;
}
/**
* Why an inflated entry exceeds the per-entry limit or the size it declared;
* undefined when the entry yielded exactly what it promised.
*/
function sizeFailure(entry, maxEntryBytes) {
	if (entry.received > maxEntryBytes) return new ExportEntryError(`${entry.info.name} yields more than the ${maxEntryBytes}-byte entry limit`, true);
	if (entry.received > entry.info.originalSize) return new ExportEntryError(`${entry.info.name} yields more bytes than the ${entry.info.originalSize} bytes it declares`, false);
}
/**
* Why one entry exceeds the per-entry limit or disagrees with its declared
* size; undefined when the entry is acceptable.
*/
function entrySizeError(entry, maxEntryBytes) {
	if (entry.originalSize > maxEntryBytes) return new ExportEntryError(`${entry.name} declares ${entry.originalSize} uncompressed bytes, more than the ${maxEntryBytes}-byte entry limit`, true);
	if (entry.size > maxEntryBytes) return new ExportEntryError(`${entry.name} stores ${entry.size} bytes, more than the ${maxEntryBytes}-byte entry limit`, true);
	if (entry.compression === 0 && entry.size !== entry.originalSize) return new ExportEntryError(`${entry.name} declares ${entry.originalSize} uncompressed bytes but stores ${entry.size} bytes`, false);
}
/**
* Promise form of fflate's asynchronous `unzip`. The filter sees each central
* directory record, including its declared `originalSize`, before anything is
* inflated.
*/
function unzipAsync(archive, filter, signal) {
	return new Promise((resolve, reject) => {
		signal.throwIfAborted();
		const onAbort = () => {
			terminate();
			reject(signal.reason);
		};
		const terminate = unzip(archive, { filter }, (error, files) => {
			signal.removeEventListener("abort", onAbort);
			if (error === null) resolve(files);
			else reject(error);
		});
		signal.addEventListener("abort", onAbort, { once: true });
	});
}
//#endregion
//#region lib/types/importer.js
/**
* Import of a complete DeepSeek chat export held in memory into a
* caller-chosen Workspace, one conversation at a time.
*
* `chat_session_index.json` selects the conversations to import. One streaming
* pass then stores the images of every chosen conversation and a second
* converts and writes each chosen conversation as its entry arrives. Nothing
* is inflated for a conversation the index did not choose.
*/
/** The archive entry naming the export format version. */
const MANIFEST_ENTRY = "manifest.json";
/** The archive entry listing every conversation with its order. */
const INDEX_ENTRY = "chat_session_index.json";
/** A conversation file; the id is the entry name. */
const CONVERSATION_ENTRY = /^chat_sessions\/([^/]+)\.json$/u;
/** An image file; the first path segment is the id of the conversation it belongs to. */
const IMAGE_ENTRY = /^chat_sessions\/([^/]+)_files\/[^/]+$/u;
const IMAGE_MEDIA_TYPES = new Map([
	[".png", "image/png"],
	[".jpg", "image/jpeg"],
	[".jpeg", "image/jpeg"],
	[".webp", "image/webp"],
	[".gif", "image/gif"]
]);
/**
* Import the conversations selected by the export index into a Workspace.
*
* Failures detected before the passes (an unreadable archive, a missing or
* unreadable manifest or index, an oversized manifest or index entry, no
* conversation) write nothing; an archive whose structure breaks during a pass
* fails with `unreadable-archive` after the Sessions delivered before the break
* were written. One streaming pass stores the images of every chosen
* conversation before any conversation is parsed; a second converts each chosen
* conversation as its entry arrives and looks the stored references up. Each
* converted conversation is seed-validated with `Session.create` before it is
* written; a conversation that fails is reported in `skipped`, the next older
* conversation the index lists takes its place, and the others continue. That
* replacement is read by name, so `ExportArchive.read` re-parses the central
* directory once for it and a further image pass collects its images; neither
* cost grows with the number of images.
* @param deps - storage, Workspace, and id services.
* @param request - archive bytes, target Workspace, limits, and progress callback.
* @param signal - cancellation, checked between steps; written Sessions remain.
* @returns the import summary.
* @throws {ExportSourceError} for an archive-level failure, or `workspace-not-found` when the Workspace is removed during the import.
*/
async function importDeepseekChatExport(deps, request, signal) {
	const { limits, workspace } = request;
	const archive = await openExportArchive(request.archive, limits.maxEntryBytes, signal);
	const skipped = [];
	const report = (item) => {
		skipped.push(item);
		deps.warn(`deepseek-chat-import: conversation ${item.conversationId} was not imported: ${item.reason}`);
	};
	const manifest = await readMetadata(archive, MANIFEST_ENTRY, {
		missing: "missing-manifest",
		damaged: "unreadable-archive"
	}, signal);
	const version = exportManifest.safeParse(parseJson(manifest));
	if (!version.success || version.data.export_format_version !== 3) throw new ExportSourceError("unsupported-version", `manifest.json must declare export_format_version 3`);
	const ranked = parseIndex(await readMetadata(archive, INDEX_ENTRY, {
		missing: "missing-index",
		damaged: "missing-index"
	}, signal));
	const total = Math.min(limits.maxSessions, ranked.length);
	const chosen = new Map(ranked.slice(0, total).map((row, rank) => [row.id, rank]));
	request.onProgress(0, total);
	const options = {
		createId: deps.createId,
		maxTitleBytes: limits.maxTitleBytes
	};
	const imported = [];
	/** Chosen conversations whose archive entry was handled, successfully or not. */
	const handled = /* @__PURE__ */ new Set();
	/** Chosen conversations still owed a written Session because an attempt failed. */
	let outstanding = 0;
	/** Rank of the next older conversation a failure may replace. */
	let next = total;
	/** A Workspace removed at any point stops the import before the next write. */
	const assertWorkspace = async () => {
		if (!deps.workspaceRegistered(workspace.id) || !await isDirectory(workspace.path)) throw new ExportSourceError("workspace-not-found", `workspace ${workspace.id} was removed during the import`);
	};
	await assertWorkspace();
	const storedImages = await storeImages(deps, archive, new Set(chosen.keys()), signal);
	signal.throwIfAborted();
	/** Write one converted conversation; false when it was reported as skipped. */
	const write = async (conversion, rank, conversationId, title) => {
		let id;
		try {
			id = await writeSession(deps, conversion, workspace.path, signal);
			await attachSession(workspace, id);
		} catch (error) {
			signal.throwIfAborted();
			report({
				conversationId,
				title,
				reason: errorText(error)
			});
			return false;
		}
		imported.push({
			id,
			rank
		});
		request.onProgress(imported.length, total);
		return true;
	};
	const importConversation = async (conversationId, data) => {
		await assertWorkspace();
		const rank = chosen.get(conversationId);
		handled.add(conversationId);
		let file;
		let conversion;
		try {
			file = parseConversation(data);
			conversion = convertWithStoredImages(deps, file, storedImages, options);
		} catch (error) {
			signal.throwIfAborted();
			outstanding += 1;
			report({
				conversationId,
				title: file?.title ?? null,
				reason: errorText(error)
			});
			return;
		}
		if (!await write(conversion, rank, conversationId, file.title)) outstanding += 1;
	};
	await archive.stream({
		select: (entry) => {
			const conversation = conversationIdOf(entry.name);
			return conversation !== void 0 && chosen.has(conversation);
		},
		reject: (entry, error) => {
			const conversation = conversationIdOf(entry.name);
			if (conversation === void 0) return;
			if (chosen.get(conversation) === void 0) return;
			handled.add(conversation);
			outstanding += 1;
			report({
				conversationId: conversation,
				title: null,
				reason: error.message
			});
		},
		entry: async (entry, data) => {
			await importConversation(conversationIdOf(entry.name), data);
		}
	}, signal);
	signal.throwIfAborted();
	for (const [conversationId] of chosen) {
		if (handled.has(conversationId)) continue;
		outstanding += 1;
		report({
			conversationId,
			title: null,
			reason: "the export has no file for this conversation"
		});
	}
	while (outstanding > 0 && next < ranked.length) {
		const row = ranked[next];
		next += 1;
		if (await attemptRow(row, next - 1)) outstanding -= 1;
	}
	imported.sort((left, right) => left.rank - right.rank);
	return {
		workspaceId: workspace.id,
		workspacePath: workspace.path,
		sessionIds: imported.map((item) => item.id),
		conversationCount: ranked.length,
		omittedByLimit: ranked.length - next,
		skipped
	};
	/**
	* Read one ranked conversation by name, collect the images it references
	* with one image pass, and write it. This path serves the conversations a
	* failure pulled in after the conversation pass; `read` re-parses the central
	* directory once for the conversation file.
	*/
	async function attemptRow(row, rank) {
		await assertWorkspace();
		const entry = conversationEntry(row.id);
		let file;
		let conversion;
		try {
			const data = await archive.read(entry, signal);
			if (data === void 0) throw new ExportEntryError(`${entry} is not in the export`, false);
			file = parseConversation(data);
			const images = imagePlan(file).size === 0 ? /* @__PURE__ */ new Map() : await storeImages(deps, archive, new Set([row.id]), signal);
			conversion = convertWithStoredImages(deps, file, images, options);
		} catch (error) {
			signal.throwIfAborted();
			report({
				conversationId: row.id,
				title: file?.title ?? null,
				reason: errorText(error)
			});
			return false;
		}
		return write(conversion, rank, row.id, file.title);
	}
}
/**
* Store the images of the chosen conversations in one streaming pass, keyed by
* entry name. Only `chat_sessions/{id}_files/…` entries whose id is one of
* `conversations` are inflated, one at a time; a conversation entry is skipped.
* Each handled entry records either the stored reference or why it was not
* stored, so the conversation pass can warn with that reason.
* @param deps - services, for the attachment store.
* @param archive - open archive.
* @param conversations - ids of the conversations whose images to store.
* @param signal - cancellation.
* @returns the outcome for each image entry name.
*/
async function storeImages(deps, archive, conversations, signal) {
	const stored = /* @__PURE__ */ new Map();
	const attachments = deps.attachments;
	if (attachments === void 0) return stored;
	await archive.stream({
		select: (entry) => {
			const conversation = imageConversationIdOf(entry.name);
			if (conversation === void 0 || !conversations.has(conversation)) return false;
			if (imageMediaTypeOf(entry.name) !== void 0) return true;
			stored.set(entry.name, {
				stored: false,
				reason: "unsupported image type"
			});
			return false;
		},
		reject: (entry, error) => {
			const conversation = imageConversationIdOf(entry.name);
			if (conversation === void 0 || !conversations.has(conversation)) return;
			stored.set(entry.name, {
				stored: false,
				reason: error.message
			});
		},
		entry: async (entry, data) => {
			const mediaType = imageMediaTypeOf(entry.name);
			try {
				const ref = await attachments.saveImage({
					data,
					mediaType,
					name: imageNameOf(entry.name)
				});
				stored.set(entry.name, {
					stored: true,
					ref
				});
			} catch (error) {
				signal.throwIfAborted();
				stored.set(entry.name, {
					stored: false,
					reason: errorText(error)
				});
			}
		}
	}, signal);
	return stored;
}
/**
* Convert one conversation with the references the image pass stored. That pass
* runs before this conversion, so an image whose conversation later fails
* conversion or `Session.create` seed validation is already stored without a
* referencing event; neither `SessionPersistence` nor the attachment store
* offers a removal.
* @param deps - services, for the attachment service and diagnostics.
* @param file - one validated conversation file.
* @param stored - image outcomes of the pass, keyed by entry name.
* @param options - id source and title limit.
* @returns the conversion, with an `image` block for every stored image.
* @throws {ExportConversationError} when the tree or a mapped fragment is invalid.
*/
function convertWithStoredImages(deps, file, stored, options) {
	if (deps.attachments === void 0) return convertConversation(file, options);
	const images = referencedImages(deps.warn, imagePlan(file), stored);
	return convertConversation(file, images.size === 0 ? options : {
		...options,
		images
	});
}
/**
* Read one metadata entry the import needs before the streaming pass.
* @param archive - open archive.
* @param name - entry name to read.
* @param reasons - failure category when the entry is absent or unreadable.
* @param signal - cancellation.
* @returns the entry bytes.
* @throws {ExportSourceError} with `too-large` for an oversized entry, and the given reason otherwise.
*/
async function readMetadata(archive, name, reasons, signal) {
	let bytes;
	try {
		bytes = await archive.read(name, signal);
	} catch (error) {
		signal.throwIfAborted();
		const entryError = error;
		throw new ExportSourceError(entryError.oversized ? "too-large" : reasons.damaged, entryError.message, { cause: error });
	}
	if (bytes === void 0) throw new ExportSourceError(reasons.missing, `the archive has no ${name}`);
	return bytes;
}
/**
* Rank the conversations the index lists, most recently updated first.
* @param bytes - `chat_session_index.json` bytes.
* @returns the distinct conversations, most recent first.
* @throws {ExportSourceError} `missing-index` when the entry is not a conversation index, `no-conversations` when it lists none.
*/
function parseIndex(bytes) {
	const parsed = exportSessionIndex.safeParse(parseJson(bytes));
	if (!parsed.success) throw new ExportSourceError("missing-index", "the archive has no readable chat_session_index.json");
	const rows = /* @__PURE__ */ new Map();
	for (const row of parsed.data) {
		if (rows.has(row.id)) continue;
		rows.set(row.id, {
			id: row.id,
			insertedAt: row.inserted_at,
			updatedAt: row.updated_at
		});
	}
	if (rows.size === 0) throw new ExportSourceError("no-conversations", "the export contains no importable conversation");
	return [...rows.values()].sort(compareRanks);
}
/** Order two index rows: most recently updated first, then most recently inserted, then by id. */
function compareRanks(left, right) {
	if (left.updatedAt !== right.updatedAt) return right.updatedAt - left.updatedAt;
	if (left.insertedAt !== right.insertedAt) return right.insertedAt - left.insertedAt;
	return left.id < right.id ? -1 : 1;
}
/** The conversation id one archive entry name carries, or undefined for another entry. */
function conversationIdOf(name) {
	return CONVERSATION_ENTRY.exec(name)?.[1];
}
/** The conversation id one image entry name carries, or undefined for another entry. */
function imageConversationIdOf(name) {
	return IMAGE_ENTRY.exec(name)?.[1];
}
/** The media type of one image entry, or undefined when its extension is not supported. */
function imageMediaTypeOf(name) {
	return IMAGE_MEDIA_TYPES.get(extname(name).toLowerCase());
}
/**
* A UUID file id starts every export image entry name, followed by `_`; older
* archives use `-` instead.
*/
const IMAGE_FILE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}[_-]/iu;
/**
* The display name of one image entry: its leaf name without the leading file
* id. The export names the entry `{file_id}_{file_name}`, so the rest is the
* original file name even when it contains underscores.
* @param name - image entry name, such as `chat_sessions/{id}_files/{file_id}_{file_name}`.
* @returns the file name without the file id.
*/
function imageNameOf(name) {
	return name.slice(name.lastIndexOf("/") + 1).replace(IMAGE_FILE_ID, "");
}
/** The archive entry name of one conversation. */
function conversationEntry(id) {
	return `chat_sessions/${id}.json`;
}
/**
* Decompress and validate one conversation file's bytes.
* @param data - complete uncompressed entry bytes.
* @returns the validated conversation.
* @throws {Error} naming the JSON or schema defect.
*/
function parseConversation(data) {
	const json = parseJson(data);
	if (json === void 0) throw new Error("invalid conversation file: not valid JSON");
	const parsed = exportSessionFile.safeParse(json);
	if (!parsed.success) throw new Error(`invalid conversation file: ${parsed.error.issues.map((issue) => issue.message).join("; ")}`);
	return parsed.data;
}
/**
* Plan the images of the imported branch by `export_path`.
* @param file - one validated conversation file.
* @returns each image's `FILE` id by `export_path`.
*/
function imagePlan(file) {
	const plan = /* @__PURE__ */ new Map();
	for (const message of selectBranch(file)) {
		if (message.role !== "USER") continue;
		for (const envelope of message.fragments) {
			const fragment = parseKnownFragment(envelope);
			if (fragment?.type !== "FILE") continue;
			for (const entry of fragment.files) {
				if (!entry.is_image || entry.export_path === null) continue;
				plan.set(entry.export_path, { fileId: entry.id });
			}
		}
	}
	return plan;
}
/**
* Map the images of one conversation to the references the image pass stored.
* An entry the pass did not handle or could not store becomes a synthetic file
* card with one warning naming the reason.
* @param warn - diagnostic for an image that was not stored.
* @param plan - images of the selected branch, keyed by `export_path`.
* @param stored - image outcomes of the pass, keyed by entry name.
* @returns the stored references keyed by export file id.
*/
function referencedImages(warn, plan, stored) {
	const images = /* @__PURE__ */ new Map();
	for (const [path, planned] of plan) {
		const outcome = stored.get(path);
		if (outcome === void 0) {
			warn(`deepseek-chat-import: image ${path} is not stored: not in the export`);
			continue;
		}
		if (!outcome.stored) {
			warn(`deepseek-chat-import: image ${path} is not stored: ${outcome.reason}`);
			continue;
		}
		images.set(planned.fileId, outcome.ref);
	}
	return images;
}
/** Record one stored Session as a Workspace member, naming the stored id when that fails. */
async function attachSession(workspace, id) {
	try {
		await workspace.attachSession(id);
	} catch (error) {
		throw new Error(`stored as Session ${id} but not added to the workspace: ${errorText(error)}`, { cause: error });
	}
}
async function writeSession(deps, conversion, cwd, signal) {
	const id = SessionId(deps.createId());
	const header = {
		version: SESSION_FORMAT_VERSION,
		id,
		createdAt: conversion.createdAt,
		cwd,
		isSeeded: false
	};
	Session.create(id, conversion.events, header);
	signal.throwIfAborted();
	const handle = await deps.persistence.create(header, { signal });
	try {
		await handle.append(conversion.events);
		await handle.flush();
	} finally {
		await handle.close();
	}
	deps.projectionCache?.coldSnapshot(header, SessionLogOffset(0), conversion.events);
	return id;
}
async function isDirectory(path) {
	try {
		return (await stat(path)).isDirectory();
	} catch (_missing) {
		return false;
	}
}
/** Decode one JSON file; undefined marks text that is not JSON, since JSON cannot encode undefined. */
function parseJson(bytes) {
	try {
		return JSON.parse(new TextDecoder().decode(bytes));
	} catch (_invalidJson) {
		return;
	}
}
function errorText(error) {
	return error instanceof Error ? error.message : String(error);
}
//#endregion
//#region lib/types/protocol.js
/** Route constants shared by the Host route and the browser uploader. */
/** Absolute route below `/api` that receives one export archive. */
const UPLOAD_PATH = "/api/deepseek-chat-import/archive";
UPLOAD_PATH.slice(1);
//#endregion
//#region lib/types/upload.js
/**
* Upload route: one authenticated POST carries the complete export archive,
* which is buffered in memory, imported, and released when the import ends.
* The answer streams `application/x-ndjson` progress lines and one outcome
* line. Nothing from the upload is written to disk.
*/
const NDJSON_HEADERS = {
	"content-type": "application/x-ndjson; charset=utf-8",
	"cache-control": "no-store, no-transform",
	"x-content-type-options": "nosniff"
};
/**
* Serve uploads one at a time through the owning service's import slot.
* @param handler - service operations behind the route.
* @returns the Fetch handler registered on the Connection route.
*/
function createUploadRoute(handler) {
	return async (request) => {
		if (request.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase() !== "application/octet-stream") return new Response("content type must be application/octet-stream", { status: 415 });
		const workspaceId = new URL(request.url).searchParams.get("workspaceId");
		if (workspaceId === null || workspaceId === "") return new Response("workspaceId is required", { status: 400 });
		const claim = handler.claim();
		if (claim === void 0) {
			await request.body?.cancel();
			return failure("busy", "another import is running");
		}
		let streaming = false;
		try {
			const workspace = handler.workspace(workspaceId);
			if (workspace === void 0) {
				await request.body?.cancel();
				return failure("workspace-not-found", `workspace ${workspaceId} does not exist`);
			}
			const cancel = new AbortController();
			const signal = AbortSignal.any([
				claim.signal,
				request.signal,
				cancel.signal
			]);
			const archive = await readBounded(request, handler.maxUploadBytes, signal);
			if (archive === void 0) return failure("too-large", `the archive exceeds ${handler.maxUploadBytes} bytes`);
			streaming = true;
			return new Response(importStream(handler, archive, workspace, claim, cancel, signal), {
				status: 200,
				headers: NDJSON_HEADERS
			});
		} finally {
			if (!streaming) claim.release();
		}
	};
}
/**
* Run one import behind the response body: progress lines, then one outcome
* line. Cancelling the body aborts the import; the claim is released when the
* import settles.
*/
function importStream(handler, archive, workspace, claim, cancel, signal) {
	let open = true;
	return new ReadableStream({
		start: (controller) => {
			const send = (line) => {
				if (open) controller.enqueue(encodeLine(line));
			};
			const end = () => {
				if (open) controller.close();
				open = false;
			};
			handler.import(archive, workspace, (done, total) => {
				send({
					type: "progress",
					done,
					total
				});
			}, signal).then((value) => {
				send({
					type: "outcome",
					outcome: {
						ok: true,
						value
					}
				});
				end();
			}, (error) => {
				if (signal.aborted) end();
				else if (error instanceof ExportSourceError) {
					send(outcomeLine(error.reason, error.message));
					end();
				} else {
					handler.failed(error);
					controller.error(error);
					open = false;
				}
			}).finally(() => {
				claim.release();
			});
		},
		cancel: () => {
			open = false;
			cancel.abort();
		}
	});
}
/**
* Read the whole body into one buffer sized from a valid `content-length`,
* growing only when the body is longer than declared; undefined as soon as it
* would exceed `limit` bytes.
*/
async function readBounded(request, limit, signal) {
	const declared = Number(request.headers.get("content-length") ?? NaN);
	if (declared > limit) {
		await request.body?.cancel();
		return;
	}
	let buffer = new Uint8Array(Number.isSafeInteger(declared) && declared >= 0 ? declared : 0);
	let total = 0;
	if (request.body !== null) {
		const reader = request.body.getReader();
		const stop = () => {
			reader.cancel(signal.reason).catch(() => void 0);
		};
		signal.addEventListener("abort", stop, { once: true });
		try {
			for (let chunk = await reader.read(); !chunk.done; chunk = await reader.read()) {
				const end = total + chunk.value.byteLength;
				if (end > limit) {
					await reader.cancel();
					return;
				}
				if (end > buffer.byteLength) {
					const grown = new Uint8Array(Math.min(limit, Math.max(end, buffer.byteLength * 2)));
					grown.set(buffer.subarray(0, total));
					buffer = grown;
				}
				buffer.set(chunk.value, total);
				total = end;
			}
		} finally {
			signal.removeEventListener("abort", stop);
		}
	}
	signal.throwIfAborted();
	return total === buffer.byteLength ? buffer : buffer.subarray(0, total);
}
function encodeLine(line) {
	return new TextEncoder().encode(`${JSON.stringify(line)}\n`);
}
function outcomeLine(reason, message) {
	return {
		type: "outcome",
		outcome: {
			ok: false,
			error: {
				reason,
				message
			}
		}
	};
}
function failure(reason, message) {
	return new Response(encodeLine(outcomeLine(reason, message)), {
		status: 200,
		headers: NDJSON_HEADERS
	});
}
//#endregion
//#region lib/types/index.js
/**
* Host plugin importing a DeepSeek chat export (chat.deepseek.com
* `deepseek_data-*.zip`) uploaded by the browser as stored DSH Sessions in a
* chosen Workspace. The upload is processed in memory and never written to disk.
* @module @deepseek-ai/dsh-deepseek-chat-import
*/
var __runInitializers = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
	function accept(f) {
		if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected");
		return f;
	}
	var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
	var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
	var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
	var _, done = false;
	for (var i = decorators.length - 1; i >= 0; i--) {
		var context = {};
		for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
		for (var p in contextIn.access) context.access[p] = contextIn.access[p];
		context.addInitializer = function(f) {
			if (done) throw new TypeError("Cannot add initializers after decoration has completed");
			extraInitializers.push(accept(f || null));
		};
		var result = (0, decorators[i])(kind === "accessor" ? {
			get: descriptor.get,
			set: descriptor.set
		} : descriptor[key], context);
		if (kind === "accessor") {
			if (result === void 0) continue;
			if (result === null || typeof result !== "object") throw new TypeError("Object expected");
			if (_ = accept(result.get)) descriptor.get = _;
			if (_ = accept(result.set)) descriptor.set = _;
			if (_ = accept(result.init)) initializers.unshift(_);
		} else if (_ = accept(result)) if (kind === "field") initializers.unshift(_);
		else descriptor[key] = _;
	}
	if (target) Object.defineProperty(target, contextIn.name, descriptor);
	done = true;
};
let DeepseekChatImportService = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _limits_decorators;
	return class DeepseekChatImportService extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_limits_decorators = [Remote];
			__esDecorate(this, null, _limits_decorators, {
				kind: "method",
				name: "limits",
				static: false,
				private: false,
				access: {
					has: (obj) => "limits" in obj,
					get: (obj) => obj.limits
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			if (_metadata) Object.defineProperty(this, Symbol.metadata, {
				enumerable: true,
				configurable: true,
				writable: true,
				value: _metadata
			});
		}
		config = __runInitializers(this, _instanceExtraInitializers);
		static inject = [
			"typert",
			"connection",
			"sessionPersistence",
			"workspaceRegistry"
		];
		static Config = Schema.object({
			maxSessions: Schema.natural().min(1).default(1e3).description("Most Sessions one import writes, choosing the conversations the export index lists as most recently updated."),
			maxUploadBytes: Schema.natural().min(1).default(512 * 1024 * 1024).description("Largest export archive, in bytes, one upload may carry; the whole archive is held in memory while it is imported."),
			maxEntryBytes: Schema.natural().min(1).default(256 * 1024 * 1024).description("Largest declared uncompressed size, in bytes, of one archive entry; larger entries are not decompressed."),
			maxTitleBytes: Schema.natural().min(1).default(80).description("Largest UTF-8 size, in bytes, of an imported title; keep it equal to the session-title maxTitleBytes.")
		});
		/** The running import, which owns the single import slot. */
		active;
		constructor(ctx, config) {
			super(ctx, "deepseekChatImport", { namespace: "deepseekChatImport" });
			this.config = config;
			const route = createUploadRoute({
				maxUploadBytes: config.maxUploadBytes,
				claim: () => this.claim(),
				workspace: (id) => ctx.workspaceRegistry.get(id),
				import: (archive, workspace, onProgress, signal) => importDeepseekChatExport({
					persistence: ctx.sessionPersistence,
					projectionCache: ctx.get("sessionProjectionCache"),
					attachments: ctx.get("attachments"),
					workspaceRegistered: (id) => ctx.workspaceRegistry.get(id) !== void 0,
					createId: () => randomUUID(),
					warn: (message) => {
						ctx.logger.warn(message);
					}
				}, {
					archive,
					workspace,
					limits: {
						maxSessions: config.maxSessions,
						maxEntryBytes: config.maxEntryBytes,
						maxTitleBytes: config.maxTitleBytes
					},
					onProgress
				}, signal),
				failed: (error) => {
					ctx.logger.error("deepseek-chat-import: import failed", error);
				}
			});
			ctx.effect(() => ctx.connection.fetch.register({
				path: UPLOAD_PATH,
				methods: ["POST"],
				requestBody: "streaming",
				fetch: route
			}), "deepseek-chat-import: upload route");
			ctx.effect(() => async () => {
				this.active?.controller.abort();
				await this.active?.settled;
			}, "deepseek-chat-import: running import");
		}
		/**
		* Report the limits the browser checks before uploading. Only the owner may
		* import, so every other principal is refused.
		* @param signal - Client cancellation or Remote withdrawal.
		* @returns the largest accepted archive size.
		* @throws {RemoteError} `deepseekChatImport/owner-only` for a caller that is not the owner.
		*/
		limits(signal) {
			signal.throwIfAborted();
			if (principalOf(this.ctx.invocation) !== void 0) throw new RemoteError("deepseekChatImport/owner-only", "only the owner may import DeepSeek chat exports", {});
			return { maxUploadBytes: this.config.maxUploadBytes };
		}
		claim() {
			if (this.active !== void 0) return void 0;
			const controller = new AbortController();
			const { promise: settled, resolve: settle } = Promise.withResolvers();
			this.active = {
				controller,
				settled
			};
			return {
				signal: controller.signal,
				release: () => {
					this.active = void 0;
					settle();
				}
			};
		}
	};
})();
//#endregion
export { ExportConversationError, ExportEntryError, ExportSourceError, DeepseekChatImportService as default };
