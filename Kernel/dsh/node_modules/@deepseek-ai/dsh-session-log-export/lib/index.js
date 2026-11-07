import Schema from "@deepseek-ai/schemastery";
import { brandString } from "@deepseek-ai/dsh-brand";
import { Zip, ZipDeflate } from "fflate";
import { SESSION_FORMAT_VERSION } from "@deepseek-ai/dsh-session";
import { sessionFormatLogFilename } from "@deepseek-ai/dsh-session-format";
import { SessionPersistenceNotFoundError } from "@deepseek-ai/dsh-session-persistence";
//#region lib/types/archive.js
/**
* Host-side session-log download: streams one ZIP archive whose files are the
* sessions' logical session logs plus every referenced attachment. Each log
* is read through a persistence read handle and serialized here as canonical
* JSONL — one header line, then one line per validated event — so every
* backend (JSONL, SQLite, future) exports identically. The root log uses the
* current generation's canonical `session[.vN].jsonl` name; each subagent
* descendant uses `subagents/<id>/session[.vN].jsonl`; each image referenced by any included log
* under `media/<attachmentId>.<ext>` (content-addressed, so one archive never
* duplicates a shared image); each generic file sits under
* `files/<prefix>/<digest>/<name>` and streams from the attachment store. No
* manifest is written. Before each live session's log read, the SessionStore
* flush barrier makes the current in-memory log durable; cold sessions need no
* barrier. Request abort and response-consumer cancellation share one producer
* signal and terminate the active compressor.
* Compression runs on the host with fflate's streaming Zip API, so the archive
* bytes are produced incrementally and the host never holds the whole archive
* in one buffer; production waits for consumer pull whenever the response queue
* reaches its byte high-water mark, so a slow consumer bounds accumulation to
* the fixed 64 KiB response queue plus one synchronous fflate push.
* @module
*/
/** Balanced default used when Session export configuration omits a compression level. */
const DEFAULT_SESSION_LOG_COMPRESSION_LEVEL = 6;
/**
* Resolve the persistence, session-query, and attachment services a log export needs.
* @param ctx - the composed host context.
* @returns the export services (absent when the deployment does not mount them).
*/
function sessionLogExportDeps(ctx) {
	return {
		sessionQuery: ctx.get("sessionQuery"),
		sessionPersistence: ctx.get("sessionPersistence"),
		attachments: ctx.get("attachments"),
		sessions: ctx.get("sessions")
	};
}
/**
* Flush one currently live session through the store's authoritative durability
* barrier immediately before its logical log is read. A cold or absent id has
* no in-memory work to flush.
* @param deps - export services, including the optional live-session store.
* @param id - the session whose artifact is about to be read.
* @param signal - optional cancellation observed around the flush barrier.
*/
async function flushLiveSessionLog(deps, id, signal) {
	signal?.throwIfAborted();
	const sessions = deps.sessions;
	if (sessions === void 0) return;
	const session = sessions.get(id);
	if (session === void 0) return;
	await sessions.flush(session);
	signal?.throwIfAborted();
}
/** The current generation's canonical base filename for every exported session log. */
const SESSION_LOG_FILENAME = sessionFormatLogFilename(SESSION_FORMAT_VERSION);
/**
* Serialize one session's logical log as canonical JSONL text: the header
* line, then one line per event, with a trailing newline.
* @param header - the session's immutable header.
* @param events - the validated committed events in seq order.
* @returns the JSONL text.
*/
function serializeSessionLog(header, events) {
	const lines = [JSON.stringify({
		type: "session",
		version: header.version,
		id: header.id,
		createdAt: header.createdAt,
		...header.cwd !== void 0 ? { cwd: header.cwd } : {},
		...header.parentSession !== void 0 ? { parentSession: header.parentSession } : {},
		isSeeded: header.isSeeded,
		...header.origin !== void 0 ? { origin: header.origin } : {},
		delegationDepth: header.delegationDepth ?? 0,
		...header.agentPreset !== void 0 ? { agentPreset: header.agentPreset } : {}
	})];
	for (const event of events) lines.push(JSON.stringify(event));
	return `${lines.join("\n")}\n`;
}
/**
* Read one session's complete logical log through a read handle and serialize
* it. The read observes the committed log only — persistence never returns a
* torn tail — and a handle read after a resolved flush observes at least the
* flushed prefix.
* @param persistence - the mounted persistence backend.
* @param id - the session to read.
* @param signal - optional cancellation forwarded to the open and read.
* @returns the serialized JSONL text, or `undefined` when the session does not exist.
*/
async function readSessionLogText(persistence, id, signal) {
	const options = signal === void 0 ? {} : { signal };
	let handle;
	try {
		handle = await persistence.open(id, "read", options);
	} catch (error) {
		if (error instanceof SessionPersistenceNotFoundError) return void 0;
		throw error;
	}
	try {
		const { events } = await handle.read(0, void 0, options);
		return serializeSessionLog(handle.header, events);
	} finally {
		await handle.close();
	}
}
/** Zip extension for each accepted raster media type. */
const MEDIA_TYPE_EXTENSIONS = {
	"image/png": "png",
	"image/jpeg": "jpg",
	"image/webp": "webp",
	"image/gif": "gif"
};
/**
* The zip path for one media object: content-addressed by the opaque
* attachment id so shared images land once and the id in the log maps back to
* the archive entry without a manifest.
* @param ref - the durable reference from a session log.
* @returns the archive path.
*/
function mediaEntryPath(ref) {
	return `media/${String(ref.attachmentId)}.${MEDIA_TYPE_EXTENSIONS[ref.mediaType]}`;
}
/** Archive path that preserves one stored file reference's digest and name. */
function fileEntryPath(ref) {
	const digest = String(ref.attachmentId).replace(/^sha256:/u, "");
	const name = ref.name.replace(/[\\/\u0000-\u001f\u007f]/gu, "_");
	const safeName = name === "." || name === ".." || name === "" ? "file" : name;
	return `files/${digest.slice(0, 2)}/${digest}/${safeName}`;
}
/**
* Collect direct attachment blocks from one declared V4 content array.
* @param content - an event or message content array.
* @param images - image dedupe map keyed by attachment id.
* @param files - file dedupe map keyed by attachment id and stored name.
*/
function collectAttachmentRefs(content, images, files) {
	if (!Array.isArray(content)) return;
	for (const value of content) {
		if (typeof value !== "object" || value === null || Array.isArray(value)) continue;
		const block = value;
		if (block.type === "image" && typeof block.attachment === "object" && block.attachment !== null) {
			const ref = block.attachment;
			images.set(String(ref.attachmentId), ref);
		}
		if (block.type === "file" && typeof block.attachment === "object" && block.attachment !== null) {
			const ref = block.attachment;
			files.set(`${String(ref.attachmentId)}\u0000${ref.name}`, ref);
		}
	}
}
/**
* Collect references only from declared first-party content fields and completed
* Assistant blocks. Unknown events and unrelated payload fields remain opaque.
* @param event - one parsed JSONL event object.
* @param images - image dedupe map keyed by attachment id.
* @param files - file dedupe map keyed by attachment id and stored name.
*/
function collectEventAttachmentRefs(event, images, files) {
	if (typeof event !== "object" || event === null || Array.isArray(event)) return;
	const row = event;
	const data = row.data;
	if (typeof data !== "object" || data === null) return;
	const carrier = data;
	switch (row.type) {
		case "user/message":
		case "tool/ptc-dispatch":
			collectAttachmentRefs(carrier.content, images, files);
			return;
		case "system/message":
		case "developer/message":
		case "tool/result":
		case "team/message/queued":
			collectAttachmentRefs(carrier.message?.content, images, files);
			return;
		case "agent/inbox/spliced": {
			const messages = carrier.inserted;
			if (!Array.isArray(messages)) return;
			for (const message of messages) {
				if (typeof message !== "object" || message === null || Array.isArray(message)) continue;
				collectAttachmentRefs(message.content, images, files);
			}
			return;
		}
		case "compaction/summary":
			collectAttachmentRefs(carrier.summary, images, files);
			collectAttachmentRefs(carrier.rawOutput, images, files);
			return;
		case "assistant/message":
			collectAttachmentRefs(carrier.message?.content, images, files);
			break;
		case "assistant/attempt": break;
		default: return;
	}
	if (carrier.stream !== void 0) {
		for (const record of carrier.stream) if (record.type === "chunk" && record.chunk?.type === "block-end") collectAttachmentRefs([record.chunk.block], images, files);
	}
}
/**
* Collect the distinct attachment references one stored artifact text names.
* Lines that fail to parse cannot reference attachments and are skipped (the
* artifact text itself is exported verbatim regardless).
* @param content - the stored artifact text.
* @returns image and file dedupe maps.
*/
function attachmentRefsInArtifact(content) {
	const images = /* @__PURE__ */ new Map();
	const files = /* @__PURE__ */ new Map();
	for (const line of content.split("\n")) {
		if (line === "") continue;
		let event;
		try {
			event = JSON.parse(line);
		} catch {
			continue;
		}
		collectEventAttachmentRefs(event, images, files);
	}
	return {
		images,
		files
	};
}
/**
* One safe zip path segment from an untrusted session id. Session ids are
* host-controlled, but the brand allows any non-empty string, so `../`, dot
* segments, and separator characters are neutralized before they can shape
* archive entries. Distinct ids may collapse onto one segment (id collision
* is impossible for the host-minted UUIDs, so no uniqueness suffix is kept).
* @param id - the raw session id.
* @returns a filesystem-safe single path segment.
*/
function safeSessionIdSegment(id) {
	return id.replace(/[^A-Za-z0-9_-]/g, "_");
}
/**
* The export archive filename for one root session.
* @param sessionId - the root session id (sanitized to one safe path segment).
* @returns the attachment filename for the session's export archive.
*/
function sessionLogZipFilename(sessionId) {
	return `dsh-session-${safeSessionIdSegment(sessionId)}.zip`;
}
/**
* Yield the export entries in zip order: the preloaded root log first, then
* every subagent descendant in lineage order (each flushed when live, read
* through a persistence read handle right before it is yielded, and dropped
* after the consumer moves on), then every distinct attachment referenced by
* the included logs. Images are read and verified as bounded stored objects;
* generic files remain streamed through the ZIP writer. The host holds at most
* one descendant log, one image, and one file chunk beyond the root.
* @param deps - the mounted export services (the caller answered 500 before this runs).
* @param rootContent - the already-serialized root log (read by the caller so
* the missing-session path can answer cleanly before streaming starts).
* @param sessionId - the root session id.
* @param includeDescendants - whether to include every subagent descendant.
* @param signal - optional cancellation forwarded to lineage, persistence, and attachment reads.
* @returns the export entries in zip order.
*/
async function* sessionLogZipEntries(deps, rootContent, sessionId, includeDescendants, signal) {
	const media = /* @__PURE__ */ new Map();
	const files = /* @__PURE__ */ new Map();
	const rememberAttachments = (content) => {
		const refs = attachmentRefsInArtifact(content);
		for (const [id, ref] of refs.images) media.set(id, ref);
		for (const [id, ref] of refs.files) files.set(id, ref);
	};
	rememberAttachments(rootContent);
	yield {
		path: SESSION_LOG_FILENAME,
		content: rootContent
	};
	if (includeDescendants) {
		const seen = new Set([sessionId]);
		const collect = async function* (nodes) {
			for (const node of nodes) {
				signal?.throwIfAborted();
				const id = node.session.header.id;
				if (seen.has(id)) continue;
				seen.add(id);
				await flushLiveSessionLog(deps, id, signal);
				const content = await readSessionLogText(deps.sessionPersistence, id, signal);
				signal?.throwIfAborted();
				if (content === void 0) throw new Error(`subagent "${id}" has no stored log`);
				rememberAttachments(content);
				yield {
					path: `subagents/${safeSessionIdSegment(id)}/${SESSION_LOG_FILENAME}`,
					content
				};
				yield* collect(node.descendants);
			}
		};
		const lineage = await deps.sessionQuery.traceSession(sessionId, signal);
		signal?.throwIfAborted();
		yield* collect(lineage.descendants);
	}
	for (const ref of media.values()) {
		signal?.throwIfAborted();
		const stored = await deps.attachments.readImage(ref, signal);
		signal?.throwIfAborted();
		yield {
			path: mediaEntryPath(ref),
			data: stored.data
		};
	}
	for (const ref of files.values()) {
		signal?.throwIfAborted();
		yield {
			path: fileEntryPath(ref),
			chunks: deps.attachments.readFileStream(ref, signal)
		};
	}
}
/** How many code units of Session-log text one zip push carries (bounded encode memory). */
const PUSH_CHUNK_CODE_UNITS = 65536;
/** How many bytes of media one zip push carries (bounded memory; images are already size-capped). */
const PUSH_CHUNK_BYTES = 65536;
/** Byte capacity retained by the response stream before ZIP production waits for pull. */
const RESPONSE_HIGH_WATER_MARK_BYTES = 65536;
/** One producer waiter released only when ReadableStream pull restores capacity. */
var ResponseCapacityGate = class {
	releasePending;
	/**
	* Wait until the response queue has positive byte capacity or cancellation wins.
	* @param controller - response controller whose desired size owns capacity.
	* @param signal - combined request/consumer cancellation.
	*/
	async wait(controller, signal) {
		signal.throwIfAborted();
		if (controller.desiredSize === null || controller.desiredSize > 0) return;
		await new Promise((resolve) => {
			const release = () => {
				this.releasePending = void 0;
				signal.removeEventListener("abort", release);
				resolve();
			};
			this.releasePending = release;
			signal.addEventListener("abort", release, { once: true });
		});
		signal.throwIfAborted();
	}
	/** Release the current producer waiter after a consumer pull. */
	pulled() {
		this.releasePending?.();
	}
};
/**
* Push one media object's bytes into a deflate stream in bounded chunks,
* waiting for consumer capacity between chunks like the artifact path does.
* @param deflate - the zip entry's deflate stream.
* @param data - the stored image bytes.
* @param controller - response queue controller.
* @param capacity - pull-driven response-capacity gate.
* @param signal - cancellation; throws when aborted.
*/
async function pushBinaryChunks(deflate, data, controller, capacity, signal) {
	let offset = 0;
	do {
		signal.throwIfAborted();
		const end = Math.min(offset + PUSH_CHUNK_BYTES, data.byteLength);
		const finalChunk = end >= data.byteLength;
		deflate.push(data.subarray(offset, end), finalChunk);
		offset = end;
		await capacity.wait(controller, signal);
	} while (offset < data.byteLength);
}
/** Push one streamed file entry without retaining its complete byte sequence. */
async function pushStreamChunks(deflate, chunks, controller, capacity, signal) {
	for await (const chunk of chunks) {
		signal.throwIfAborted();
		if (chunk.byteLength === 0) continue;
		deflate.push(chunk, false);
		await capacity.wait(controller, signal);
	}
	signal.throwIfAborted();
	deflate.push(new Uint8Array(), true);
	await capacity.wait(controller, signal);
}
/**
* Push one artifact's text into a deflate stream in bounded chunks, never
* splitting a surrogate pair across a chunk boundary (a lone high surrogate
* re-encodes as U+FFFD and would silently corrupt the exported artifact).
* @param deflate - the zip entry's deflate stream.
* @param content - the canonical Session-log text.
* @param controller - response queue controller.
* @param capacity - pull-driven response-capacity gate.
* @param signal - cancellation; throws when aborted.
*/
async function pushArtifactChunks(deflate, content, controller, capacity, signal) {
	const encoder = new TextEncoder();
	let offset = 0;
	let finalChunk;
	do {
		signal.throwIfAborted();
		let end = Math.min(offset + PUSH_CHUNK_CODE_UNITS, content.length);
		if (end < content.length && end - offset > 1) {
			const last = content.charCodeAt(end - 1);
			if (last >= 55296 && last <= 56319) end -= 1;
		}
		finalChunk = end >= content.length;
		deflate.push(encoder.encode(content.slice(offset, end)), finalChunk);
		offset = end;
		await capacity.wait(controller, signal);
	} while (!finalChunk);
}
/**
* Stream one session-log ZIP as a WHATWG ReadableStream. The root log is read
* and serialized by the caller before this is called (a missing root or
* missing services answer cleanly before any byte is produced); each entry is
* then encoded and deflated in bounded chunks as it is produced, so the
* archive bytes arrive incrementally. A descendant that fails to read errors
* the stream (fail-loud, never silent under-export).
* @param deps - the mounted export services (the caller answered 500 before this runs).
* @param rootContent - the already-serialized root log (first zip entry).
* @param sessionId - the root session id.
* @param includeDescendants - whether to include every subagent descendant.
* @param compressionLevel - validated fflate DEFLATE level for every ZIP entry.
* @param signal - request cancellation combined with response-consumer cancellation.
* @returns the zip byte stream.
*/
function streamSessionLogZip(deps, rootContent, sessionId, includeDescendants, compressionLevel, signal) {
	const consumerAbort = new AbortController();
	const producerSignal = AbortSignal.any([signal, consumerAbort.signal]);
	let zip;
	let zipTerminated = false;
	const capacity = new ResponseCapacityGate();
	const terminateZip = () => {
		if (zip === void 0 || zipTerminated) return;
		zipTerminated = true;
		zip.terminate();
	};
	return new ReadableStream({
		start(controller) {
			const archive = new Zip((error, data, final) => {
				/* v8 ignore next 3 -- fflate reports only internal zip failures, unreachable for valid inputs */
				if (error) {
					controller.error(error);
					return;
				}
				/* v8 ignore next -- fflate may emit empty chunks; not controllable from tests */
				if (data.byteLength > 0) controller.enqueue(data);
				if (final) controller.close();
			});
			zip = archive;
			(async () => {
				try {
					for await (const entry of sessionLogZipEntries(deps, rootContent, sessionId, includeDescendants, producerSignal)) {
						const deflate = new ZipDeflate(entry.path, { level: compressionLevel });
						archive.add(deflate);
						if ("content" in entry) await pushArtifactChunks(deflate, entry.content, controller, capacity, producerSignal);
						else if ("data" in entry) await pushBinaryChunks(deflate, entry.data, controller, capacity, producerSignal);
						else await pushStreamChunks(deflate, entry.chunks, controller, capacity, producerSignal);
					}
					archive.end();
				} catch (error) {
					/* v8 ignore next -- typed backends reject with Error, and DOMException is one in Node */
					terminateZip();
					controller.error(error instanceof Error ? error : new Error(String(error)));
				}
			})();
		},
		pull() {
			capacity.pulled();
		},
		cancel(reason) {
			consumerAbort.abort(reason instanceof Error ? reason : /* @__PURE__ */ new Error("session log export stream cancelled"));
			terminateZip();
		}
	}, {
		highWaterMark: RESPONSE_HIGH_WATER_MARK_BYTES,
		size: (chunk) => chunk.byteLength
	});
}
//#endregion
//#region lib/types/markdown.js
/**
* Pure backend converter for one complete committed Session log. The Session
* Markdown copy Host route uses it, and the package entry exports it for
* callers that already hold the logical events. The raw log is authoritative:
* compaction surface replacements never remove earlier messages, and
* fork-inherited history appears once. Reasoning, system and developer
* messages, injected non-human context, request headers, and runtime records
* are omitted.
* @module
*/
/** Heading used when the log carries no title and the Session has no name. */
const UNTITLED_SESSION_MARKDOWN_TITLE = "Untitled Session";
/**
* Render one complete committed Session log as Markdown. This is a pure backend
* utility: it needs no mounted plugin, reads no storage, and does not mutate the
* supplied events. The latest `session/title` event supplies the heading; the
* rest of the transcript is derived only from the supplied logical events.
* @param events - complete committed logical event snapshot in seq order, including any inherited prefix.
* @param sourcePath - optional backend-resolved path to the selected Session artifact; emitted verbatim in a fenced text block.
* @returns Markdown ending in exactly one newline; an empty log returns the heading and optional source metadata only.
*/
function formatSessionMarkdown(events, sourcePath) {
	const title = events.findLast((event) => event.type === "session/title")?.data.title;
	const sections = [`# ${normalizedTitle(title)}`];
	if (sourcePath !== void 0 && sourcePath !== "") sections.push(`Session file:

${fenced(sourcePath, "text", false)}`);
	for (const turn of collectTurns(events)) {
		const finalIndex = finalReplyIndex(turn.segments);
		const end = finalIndex < 0 ? turn.segments.length : finalIndex;
		let pending = [];
		for (const segment of turn.segments.slice(0, end)) if (segment.kind === "user") {
			if (pending.length > 0) {
				sections.push(collapsibleSection(pending));
				pending = [];
			}
			sections.push(quote(segment.text));
		} else pending.push(segment);
		if (pending.length > 0) sections.push(collapsibleSection(pending));
		const final = turn.segments[finalIndex];
		const finalText = final?.kind === "assistant" ? closeUnfinishedFence(trimBlankEdgeLines(final.text)) : "";
		if (finalText !== "") sections.push(finalText);
	}
	return `${sections.join("\n\n").trimEnd()}\n`;
}
function normalizedTitle(title) {
	const line = title?.replace(/\r\n?|\n/gu, " ").trim() ?? "";
	return line === "" ? UNTITLED_SESSION_MARKDOWN_TITLE : line;
}
function collectTurns(events) {
	const turns = [];
	let current;
	const begin = () => {
		current = {
			segments: [],
			calls: /* @__PURE__ */ new Map()
		};
		turns.push(current);
		return current;
	};
	const active = () => current ?? begin();
	for (const event of events) switch (event.type) {
		case "turn/start":
			begin();
			break;
		case "turn/end":
			current = void 0;
			break;
		case "user/message": {
			if (event.surfaceOp !== "append" || event.data.source.kind !== "user") break;
			const text = visibleContent(event.data.content);
			if (text !== "") active().segments.push({
				kind: "user",
				text
			});
			break;
		}
		case "assistant/message": {
			if (event.surfaceOp !== "append") break;
			const message = event.data.message;
			const text = assistantText(message);
			const toolCalls = message.content.some((block) => block.type === "tool-call");
			if (trimBlankEdgeLines(text) !== "") active().segments.push({
				kind: "assistant",
				text,
				toolCalls
			});
			break;
		}
		case "tool/call": {
			const turn = active();
			const entry = {
				kind: "tool",
				name: event.data.name,
				arguments: event.data.arguments,
				result: void 0
			};
			turn.segments.push(entry);
			turn.calls.set(String(event.data.callId), entry);
			break;
		}
		case "tool/result": {
			if (event.surfaceOp !== "append") break;
			const turn = active();
			const callId = String(event.data.message.toolCallId);
			let entry = turn.calls.get(callId);
			if (entry === void 0) {
				entry = {
					kind: "tool",
					name: "unknown",
					arguments: "",
					result: void 0
				};
				turn.segments.push(entry);
				turn.calls.set(callId, entry);
			}
			entry.result = toolResult(event.data);
			break;
		}
		default: break;
	}
	return turns.filter((turn) => turn.segments.length > 0);
}
function finalReplyIndex(segments) {
	const index = segments.length - 1;
	const entry = segments[index];
	return entry?.kind === "assistant" && !entry.toolCalls ? index : -1;
}
function collapsibleSection(entries) {
	return `<details><summary>${entries.length === 1 ? "1 previous message" : `${String(entries.length)} previous messages`}</summary>\n\n${entries.map(renderEntry).join("\n\n")}\n\n</details>`;
}
function renderEntry(entry) {
	if (entry.kind === "assistant") return `**Assistant**\n\n${escapeWrapperTagsOutsideFences(closeUnfinishedFence(trimBlankEdgeLines(entry.text)))}`;
	const heading = `**Tool: \`${escapeWrapperTags(entry.name)}\`**`;
	const argumentsText = entry.arguments === "" ? "(no arguments)" : fenced(jsonArguments(entry.arguments), "json");
	const result = entry.result;
	const resultHeading = result?.isError === true ? "**Result (error)**" : "**Result**";
	let resultBody = "";
	if (result === void 0) resultBody = "(no result recorded)";
	else if (result.reason !== void 0 && result.reason !== "") {
		resultBody = `Error: ${escapeWrapperTags(result.reason)}`;
		if (result.text !== "") resultBody += `\n\n${fenced(result.text)}`;
	} else if (result.text === "") resultBody = "(no output)";
	else resultBody = fenced(result.text);
	return `${heading}\n\n${argumentsText}\n\n${resultHeading}\n\n${resultBody}`;
}
function visibleContent(content) {
	const parts = [];
	for (const block of content) switch (block.type) {
		case "text":
			if (block.text !== "") parts.push(block.text);
			break;
		case "image":
			parts.push(imageLabel(block.attachment));
			break;
		case "file":
			parts.push(fileLabel(block.attachment));
			break;
		default: break;
	}
	return parts.join("\n\n");
}
function assistantText(message) {
	return visibleContent(message.content);
}
function toolResult(data) {
	return {
		text: visibleContent(data.message.content),
		isError: data.message.isError === true,
		reason: data.error?.reason
	};
}
function quote(text) {
	return text.replace(/\r\n?/gu, "\n").replace(/\n+$/u, "").split("\n").map((line) => line === "" ? ">" : `> ${line}`).join("\n");
}
function imageLabel(ref) {
	return `[Image: ${labelName(ref.name)} (${String(ref.width)}×${String(ref.height)}, ${String(ref.bytes)} bytes)]`;
}
function fileLabel(ref) {
	return `[File: ${labelName(ref.name)} (${String(ref.bytes)} bytes)]`;
}
function labelName(name) {
	const normalized = name?.replace(/\s+/gu, " ").replace(/\]/gu, "\\]").trim() ?? "";
	return normalized === "" ? "attachment" : normalized;
}
function jsonArguments(argumentsText) {
	let parsed;
	try {
		parsed = JSON.parse(argumentsText);
	} catch {
		return argumentsText;
	}
	return JSON.stringify(parsed, null, 2);
}
function fenced(text, language = "", trimTrailing = true) {
	const content = trimTrailing ? text.replace(/\n+$/u, "") : text;
	let longest = 0;
	for (const match of content.matchAll(/`+/gu)) longest = Math.max(longest, match[0].length);
	const fence = "`".repeat(Math.max(3, longest + 1));
	return `${fence}${language}\n${content}\n${fence}`;
}
function trimBlankEdgeLines(text) {
	const lines = text.replace(/\r\n?/gu, "\n").split("\n");
	let start = 0;
	let end = lines.length;
	while (start < end && lines[start]?.trim() === "") start += 1;
	while (end > start && lines[end - 1]?.trim() === "") end -= 1;
	return lines.slice(start, end).join("\n");
}
function scanMarkdown(markdown) {
	const lines = markdown.replace(/\r\n?/gu, "\n").split("\n");
	const scanned = [];
	let fence;
	let indented = false;
	for (const line of lines) {
		if (fence !== void 0) {
			scanned.push({
				text: line,
				code: true
			});
			if (closesFence(line, fence)) fence = void 0;
			continue;
		}
		const opening = openingFence(line);
		if (opening !== void 0) {
			fence = opening;
			indented = false;
			scanned.push({
				text: line,
				code: true
			});
			continue;
		}
		if (/^(?: {4}|\t)/u.test(line)) {
			indented = true;
			scanned.push({
				text: line,
				code: true
			});
			continue;
		}
		scanned.push({
			text: line,
			code: indented && line.trim() === ""
		});
		if (line.trim() !== "") indented = false;
	}
	return {
		lines: scanned,
		openFence: fence
	};
}
function openingFence(line) {
	const opening = /^ {0,3}(`{3,}|~{3,})(.*)$/u.exec(line);
	if (opening === null) return void 0;
	const marker = String(opening[1]);
	const info = String(opening[2]);
	if (marker.startsWith("`") && info.includes("`")) return void 0;
	return {
		character: marker.slice(0, 1),
		length: marker.length
	};
}
function closesFence(line, fence) {
	return new RegExp(`^ {0,3}${fence.character}{${String(fence.length)},}\\s*$`, "u").test(line);
}
function closeUnfinishedFence(markdown) {
	const scan = scanMarkdown(markdown);
	if (scan.openFence === void 0) return markdown;
	return `${markdown.replace(/\n+$/u, "")}\n${scan.openFence.character.repeat(scan.openFence.length)}`;
}
function escapeWrapperTagsOutsideFences(markdown) {
	return scanMarkdown(markdown).lines.map((line) => line.code ? line.text : escapeWrapperTags(line.text)).join("\n");
}
function escapeWrapperTags(text) {
	return text.replace(/<(?=\/?(?:details|summary)(?=[\s/>]))/giu, "&lt;");
}
//#endregion
//#region lib/types/routes.js
/**
* The absolute pathnames the Host registers the export routes under and the
* document-relative forms the browser addresses; see
* .agents/notes/implemented/architecture/2026-09-14-web-document-relative-app-routes.md.
*/
/** Absolute registration path for the ZIP download route. */
const SESSION_LOG_EXPORT_PATH = "/api/session.export";
SESSION_LOG_EXPORT_PATH.slice(1);
/** Absolute registration path for the Markdown copy route. */
const SESSION_MARKDOWN_EXPORT_PATH = "/api/session.markdown";
SESSION_MARKDOWN_EXPORT_PATH.slice(1);
//#endregion
//#region lib/types/index.js
/** Session export command and Host-owned ZIP download and Markdown copy routes. */
const name = "session-log-download";
const inject = ["commands", "connection"];
/** Validate Session-log archive configuration. */
const Config = Schema.object({ compressionLevel: Schema.number().step(1).min(0).max(9).default(6) });
const REQUESTED = {
	kind: "success",
	text: "Session log download requested."
};
/**
* Register the `/export` command and authenticated ZIP download and Markdown copy routes.
* @param ctx - Host context carrying the human-command registry.
* @param config - resolved compression policy.
*/
function apply(ctx, config = {}) {
	ctx.effect(() => ctx.commands.register({
		definitionId: brandString("@deepseek-ai/dsh-session-log-export"),
		name: "export",
		description: "Download this Session log as a ZIP archive",
		handler: (invocation) => Promise.resolve(invocation.rawInput.trim() === "" ? REQUESTED : {
			kind: "error",
			text: "The Web /export command does not accept a path."
		})
	}), "session-log-download: command");
	connectionOf(ctx).fetch.register({
		path: SESSION_LOG_EXPORT_PATH,
		methods: ["GET", "HEAD"],
		requestBody: "buffered",
		fetch: async (request) => {
			const response = await sessionLogExportResponse(ctx, request, config.compressionLevel ?? 6);
			if (request.method === "GET") return response;
			await response.body?.cancel();
			return new Response(null, {
				status: response.status,
				headers: response.headers
			});
		}
	});
	connectionOf(ctx).fetch.register({
		path: SESSION_MARKDOWN_EXPORT_PATH,
		methods: ["GET", "HEAD"],
		requestBody: "buffered",
		fetch: async (request) => {
			const response = await sessionMarkdownResponse(ctx, request);
			if (request.method === "GET") return response;
			await response.body?.cancel();
			return new Response(null, {
				status: response.status,
				headers: response.headers
			});
		}
	});
}
function connectionOf(ctx) {
	return Reflect.get(ctx, "connection");
}
/**
* Produce one complete Session transcript as Markdown from one Session
* observation. The read is a detached export observation through
* session-query's live-preferred corpus, so it creates no Agent, retains no
* Session, and writes no Session data. The formatter reads the latest title from
* the same event snapshot as the body. Abort is observed before and after the
* read; the corpus read itself has no cancellation parameter.
* @param ctx - host context carrying Connection and session-query.
* @param request - authenticated Fetch request with a required `sessionId` query.
* @returns the Markdown response, or a sanitized status for invalid, missing, or unavailable reads.
*/
async function sessionMarkdownResponse(ctx, request) {
	const sessionIdValue = new URL(request.url).searchParams.get("sessionId");
	if (sessionIdValue === null || sessionIdValue.length === 0) return new Response("missing or invalid sessionId query parameter", { status: 400 });
	const sessionQuery = ctx.get("sessionQuery");
	if (sessionQuery === void 0) return new Response("session markdown export is unavailable: missing session-query service", { status: 500 });
	const sessionId = brandString(sessionIdValue);
	try {
		request.signal.throwIfAborted();
		const snapshot = await sessionQuery.readSession(sessionId);
		request.signal.throwIfAborted();
		const metadata = await ctx.get("sessionPersistence")?.stat(sessionId, { signal: request.signal });
		request.signal.throwIfAborted();
		return new Response(formatSessionMarkdown(snapshot.events, metadata?.location?.path), { headers: { "content-type": "text/markdown; charset=utf-8" } });
	} catch (error) {
		request.signal.throwIfAborted();
		if (isSessionNotFound(error)) return new Response("session not found", { status: 404 });
		return new Response("session markdown export failed to read the stored log", { status: 500 });
	}
}
/**
* Identify the session-query "not found" failure by its stable machine code.
* @param error - an arbitrary rejection from the query service.
* @returns whether the error reports a missing Session.
*/
function isSessionNotFound(error) {
	return error instanceof Error && "code" in error && error.code === "SESSION_QUERY_SESSION_NOT_FOUND";
}
async function sessionLogExportResponse(ctx, request, compressionLevel) {
	const url = new URL(request.url);
	const query = Object.fromEntries(url.searchParams);
	const sessionIdValue = query["sessionId"];
	const descendantsValue = query["includeDescendants"];
	if (sessionIdValue === void 0 || sessionIdValue.length === 0 || descendantsValue !== void 0 && descendantsValue !== "true" && descendantsValue !== "false") return new Response("missing or invalid sessionId query parameter", { status: 400 });
	const sessionId = brandString(sessionIdValue);
	const deps = sessionLogExportDeps(ctx);
	if (deps.sessionQuery === void 0 || deps.sessionPersistence === void 0 || deps.attachments === void 0) return new Response("session log export is unavailable: missing session-query, session-persistence, or attachments service", { status: 500 });
	const ready = {
		sessionQuery: deps.sessionQuery,
		sessionPersistence: deps.sessionPersistence,
		attachments: deps.attachments,
		sessions: deps.sessions
	};
	let rootContent;
	try {
		await flushLiveSessionLog(deps, sessionId, request.signal);
		rootContent = await readSessionLogText(deps.sessionPersistence, sessionId, request.signal);
		request.signal.throwIfAborted();
	} catch {
		request.signal.throwIfAborted();
		return new Response("session log export failed to read the stored log", { status: 500 });
	}
	if (rootContent === void 0) return new Response("session not found", { status: 404 });
	return new Response(streamSessionLogZip(ready, rootContent, sessionId, descendantsValue === "true", compressionLevel, request.signal), { headers: {
		"content-type": "application/zip",
		"content-disposition": `attachment; filename="${sessionLogZipFilename(sessionId)}"`
	} });
}
//#endregion
export { Config, DEFAULT_SESSION_LOG_COMPRESSION_LEVEL, SESSION_LOG_EXPORT_PATH, SESSION_LOG_FILENAME, apply, flushLiveSessionLog, formatSessionMarkdown, inject, name, readSessionLogText, serializeSessionLog, sessionLogExportDeps, sessionLogZipEntries, sessionLogZipFilename, streamSessionLogZip };
