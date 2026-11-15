import z from "@deepseek-ai/schemastery";
import { scopeOf } from "@deepseek-ai/dsh-scope";
import { MAX_TIMER_DELAY_MS } from "@deepseek-ai/dsh-timeout";
import { Client, StreamableHTTPClientTransport, specTypeSchemas } from "@modelcontextprotocol/client";
import { assertNever, deepEqualJson } from "@deepseek-ai/dsh-util-values";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import { scrubbedParentEnv } from "@deepseek-ai/dsh-subprocess";
import { createHash } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import { isImageAdmissionError } from "@deepseek-ai/dsh-attachment";
import { assertSupportedJsonSchema } from "@deepseek-ai/dsh-tools";
//#region lib/types/diagnostic.js
/**
* Connection diagnostics retain cause chains. Configurations that resolve
* secrets replace every known secret value before text leaves the instance.
*/
/** Local failure whose message contains no expanded configuration or upstream text. */
var McpLocalError = class extends Error {};
/** Placeholder written in place of a known secret value. */
const REDACTED = "[redacted]";
/**
* Replace every occurrence of known secret values, longest first so a secret
* that contains another is replaced whole.
* @param text - diagnostic or upstream text.
* @param secrets - non-empty secret values known to the instance.
* @returns the text with each occurrence replaced by `[redacted]`.
*/
function redactSecrets(text, secrets) {
	let out = text;
	for (const secret of [...secrets].sort((left, right) => right.length - left.length)) out = out.replaceAll(secret, REDACTED);
	return out;
}
/** One readable segment of the chain; generic `Error` names are noise and dropped. */
function segmentOf(value) {
	if (!(value instanceof Error)) return String(value);
	const base = value.name === "Error" || value.message === "" ? value.message === "" ? value.name : value.message : `${value.name}: ${value.message}`;
	if (base !== "" && base !== "Error") return base;
	const code = value.code;
	return typeof code === "string" ? code : value.name;
}
/**
* Flatten an error and its `cause` chain into `outer, cause: inner, …`.
* @param error - the caught value; non-Errors stringify as-is.
* @returns the joined diagnostic line.
*/
function describeError(error) {
	const seen = /* @__PURE__ */ new Set();
	const segments = [];
	let current = error;
	while (current !== void 0 && current !== null && !seen.has(current)) {
		seen.add(current);
		segments.push(segments.length === 0 && current instanceof Error ? String(current) : segmentOf(current));
		current = current instanceof Error ? current.cause : void 0;
	}
	const head = segments[0];
	if (head === void 0) return String(error);
	return head + segments.slice(1).map((segment) => `, cause: ${segment}`).join("");
}
//#endregion
//#region lib/types/deadline.js
/** Shared settlement deadline for the startup and probe budgets. */
/**
* Settle with `promise`, or reject with `timeoutMessage` once `remainingMs`
* elapses first. The timer never holds the process open, and a non-Error
* rejection is wrapped so callers always observe an Error.
* @param promise - operation the deadline bounds.
* @param remainingMs - remaining budget in milliseconds (clamped to at least 1).
* @param timeoutMessage - diagnostic carried by the deadline rejection.
* @returns the operation's value; rejects with the deadline or the operation's failure.
*/
function withDeadline(promise, remainingMs, timeoutMessage) {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => {
			reject(new McpLocalError(timeoutMessage));
		}, Math.max(1, remainingMs));
		timer.unref();
		promise.then((value) => {
			clearTimeout(timer);
			resolve(value);
		}, (error) => {
			clearTimeout(timer);
			reject(error instanceof Error ? error : new Error(String(error)));
		});
	});
}
//#endregion
//#region lib/types/transport.js
/**
* Transport factory: creates the appropriate MCP transport based on the
* plugin's resolved config. Stdio spawns a child process (with credential
* scrubbing); Streamable HTTP connects to a URL.
*
* @module
*/
/**
* The subprocess seam's scrubbed parent env (credential-shaped and stale
* `DSH_*` names dropped), plus the spec's explicit env. The MCP SDK owns the
* actual spawn, so this transport shares the scrub definition rather than the
* spawn path.
*/
function buildChildEnv(extra) {
	return {
		...scrubbedParentEnv(),
		...extra
	};
}
/**
* Create an MCP transport from the resolved plugin config.
*
* @param config - Resolved plugin config discriminated on `transport`.
* @returns an unstarted MCP transport (stdio or Streamable HTTP).
*/
function createTransport(config) {
	switch (config.transport) {
		case "stdio": return new StdioClientTransport({
			command: config.command,
			args: config.args,
			env: buildChildEnv(config.env),
			cwd: config.cwd
		});
		case "streamable-http": return new StreamableHTTPClientTransport(new URL(config.url), { requestInit: { headers: config.headers } });
	}
}
//#endregion
//#region lib/types/generation.js
/** One transport lifetime, including pending work and confirmed closure. */
/** Tracks actual tasks separately from their interruptible callers. */
var McpGeneration = class {
	client;
	closeTimeoutMs;
	diagnostic;
	/** Cancellation shared by the transport and all work owned by this generation. */
	lifetime = new AbortController();
	pending = /* @__PURE__ */ new Set();
	closed = Promise.withResolvers();
	transport;
	started = false;
	closeObserved = false;
	closing;
	constructor(client, closeTimeoutMs, diagnostic) {
		this.client = client;
		this.closeTimeoutMs = closeTimeoutMs;
		this.diagnostic = diagnostic;
	}
	/**
	* Connect within the caller's remaining startup or probe budget.
	* @param config - resolved transport configuration.
	* @param timeoutMs - remaining operation budget; undefined leaves the SDK default.
	* @returns completion of negotiation, or cancellation/deadline rejection.
	*/
	connect(config, timeoutMs) {
		this.lifetime.signal.throwIfAborted();
		const transport = this.transport = createTransport(config);
		const close = transport.close.bind(transport);
		let closing;
		transport.close = () => closing ??= close();
		const start = transport.start.bind(transport);
		transport.start = () => {
			this.lifetime.signal.throwIfAborted();
			this.started = true;
			return start();
		};
		transport.onclose = () => {
			this.closeObserved = true;
			this.closed.resolve();
		};
		return this.run(async (signal) => {
			await this.client.connect(transport, {
				signal,
				...timeoutMs === void 0 ? {} : { timeout: Math.max(1, timeoutMs) }
			});
			this.assertOpen();
		}, timeoutMs);
	}
	/** Reject when this generation was cancelled or its transport reported closure. */
	assertOpen() {
		this.lifetime.signal.throwIfAborted();
		if (this.closeObserved) throw new McpLocalError("MCP transport closed");
	}
	/**
	* Track work through settlement, even when its caller times out or is cancelled.
	* @param operation - work receiving this generation's cancellation signal.
	* @param timeoutMs - optional caller deadline in milliseconds; does not cancel the work.
	* @returns the operation's result, or a cancellation/deadline rejection.
	*/
	run(operation, timeoutMs) {
		const signal = this.lifetime.signal;
		const task = Promise.resolve().then(() => {
			signal.throwIfAborted();
			return operation(signal);
		});
		this.pending.add(task);
		task.then(() => this.pending.delete(task), () => this.pending.delete(task));
		const interrupted = new Promise((resolve, reject) => {
			const abort = () => {
				reject(signal.reason);
			};
			signal.addEventListener("abort", abort, { once: true });
			if (signal.aborted) abort();
			task.then(resolve, reject).finally(() => {
				signal.removeEventListener("abort", abort);
			});
		});
		return timeoutMs === void 0 ? interrupted : withDeadline(interrupted, timeoutMs, "MCP operation timed out");
	}
	/**
	* Cancel callers, close the transport, and await owned work within the cleanup budget.
	* @returns the shared cleanup result, distinguishing transport closure from cleanup failure.
	*/
	close() {
		return this.closing ??= this.finish();
	}
	async finish() {
		this.lifetime.abort(new McpLocalError("MCP connection closed"));
		let cleanupError;
		const work = async () => {
			try {
				await (this.client.transport === void 0 ? this.transport?.close() : this.client.close());
			} catch (error) {
				cleanupError = this.diagnostic(error);
			}
			await Promise.allSettled([...this.pending]);
			if (this.started && !this.closeObserved) await this.closed.promise;
		};
		try {
			await withDeadline(work(), this.closeTimeoutMs, "MCP cleanup did not settle");
			return {
				closeConfirmed: true,
				...cleanupError === void 0 ? {} : { cleanupError }
			};
		} catch (error) {
			return {
				closeConfirmed: this.transport === void 0 || this.closeObserved,
				cleanupError: cleanupError ?? this.diagnostic(error)
			};
		}
	}
};
//#endregion
//#region lib/types/templates.js
/**
* `${credential:REF}` syntax and substitution for MCP transport strings.
* Managers validate syntax at configuration writes; the client resolves values
* at runtime without changing the stored configuration.
*
* @module
*/
/** A referenced credential has no configured value. */
var MissingTemplateValue = class extends McpLocalError {};
const TOKEN = /\$\{credential:([A-Za-z_][A-Za-z0-9_]*)\}/g;
/** One whole occurrence must BE a token; containment is not enough. */
const TOKEN_EXACT = /^\$\{credential:[A-Za-z_][A-Za-z0-9_]*\}$/;
/** Any `${…}` occurrence, well-formed or not, for fail-loud validation. */
const SUSPECT = /\$\{[^}]*\}/g;
/**
* The credential references one configured string names, read statically. A
* rotation observer uses this over the raw configuration so a server whose
* earlier resolution failed — a credential configured only now — is still
* recognized as affected.
* @param value - the configured string.
* @returns the referenced credential names, in template order.
*/
function credentialRefsIn(value) {
	return [...value.matchAll(TOKEN)].map((token) => token[1]);
}
/**
* Reject a string whose `${…}` occurrences are not all well-formed template
* tokens, naming the offending occurrence and where it sits.
* @param value - the configured string.
* @param where - configuration path used in the thrown message.
*/
function assertTemplateSyntax(value, where) {
	for (const occurrence of value.matchAll(SUSPECT)) if (!TOKEN_EXACT.test(occurrence[0])) throw new Error(`${where}: "${occurrence[0]}" is not a template token — use \${credential:REF}`);
}
/**
* Resolve every template token in one string.
* @param value - the configured string.
* @param where - configuration path used in thrown messages.
* @param credential - credential-reference lookup returning `undefined` while unconfigured.
* @returns the substituted string.
* @throws when a referenced credential is unconfigured.
*/
async function resolveTemplateString(value, where, credential) {
	let out = "";
	let cursor = 0;
	for (const token of value.matchAll(TOKEN)) {
		const name = token[1];
		const resolved = await credential(name);
		if (resolved === void 0) throw new MissingTemplateValue(`${where}: credential "${name}" is not configured — store it through the credential seam or remove the \${credential:${name}} template`);
		out += value.slice(cursor, token.index) + resolved;
		cursor = token.index + token[0].length;
	}
	return out + value.slice(cursor);
}
//#endregion
//#region lib/types/credentials.js
/** Runtime template resolution and secret redaction for one MCP instance. */
/** Resolves raw configuration through the current credential provider and redacts every value it resolved. */
var McpTemplateBinding = class {
	ctx;
	config;
	/** Enabled credential references from raw configuration, including currently unconfigured references. */
	refs;
	/** Resolved values and their URL-encoded forms, retained for the instance lifetime. */
	secrets = /* @__PURE__ */ new Set();
	constructor(ctx, config) {
		this.ctx = ctx;
		this.config = config;
		const strings = config.transport === "stdio" ? [
			config.command,
			...config.args,
			...Object.values(config.env),
			config.cwd
		] : [config.url, ...Object.values(config.headers)];
		this.refs = new Set(config.expandTemplates ? strings.flatMap(credentialRefsIn) : []);
		if (config.expandTemplates) {
			for (const value of strings) assertTemplateSyntax(value, `mcp-client(${config.serverName})`);
			if (this.refs.size > 0 && ctx.get("credentials") === void 0) throw new McpLocalError("MCP credential templates require a credentials provider");
		}
	}
	/**
	* Replace every value this binding resolved, including earlier rotations.
	* @param text - diagnostic or upstream text.
	* @returns the text with resolved values replaced by `[redacted]`.
	*/
	redact(text) {
		return redactSecrets(text, this.secrets);
	}
	/**
	* Format a failure, naming the operation and redacting resolved values when templates are enabled.
	* @param error - caught operation failure.
	* @param operation - local operation name for the diagnostic.
	* @returns diagnostic text safe for the configured template mode.
	*/
	diagnostic(error, operation) {
		if (!this.config.expandTemplates) return describeError(error);
		return `mcp-client(${this.config.serverName}): ${operation}: ${this.redact(describeError(error))}`;
	}
	/**
	* Resolve transport strings without changing the plugin's raw configuration.
	* @param signal - cancellation checked before and after template resolution.
	* @returns resolved transport configuration, or the raw configuration when expansion is disabled.
	* @throws when a credential or the credentials provider is unavailable.
	*/
	async resolve(signal) {
		signal.throwIfAborted();
		if (!this.config.expandTemplates) return this.config;
		const values = /* @__PURE__ */ new Map();
		const credential = (ref) => {
			let value = values.get(ref);
			if (value === void 0) {
				const provider = this.ctx.get("credentials");
				if (provider === void 0) throw new McpLocalError("MCP credential templates require a credentials provider");
				value = provider.resolve(ref).then((result) => {
					if (result !== void 0 && result.value !== "") this.secrets.add(result.value).add(encodeURIComponent(result.value));
					return result?.value;
				});
				values.set(ref, value);
			}
			return value;
		};
		const resolve = async (value, path) => {
			const result = await resolveTemplateString(value, path, credential);
			signal.throwIfAborted();
			return result;
		};
		const record = async (value, path) => {
			const resolved = [];
			for (const [key, entry] of Object.entries(value)) resolved.push([key, await resolve(entry, `${path}.${key}`)]);
			return Object.fromEntries(resolved);
		};
		const config = this.config;
		if (config.transport === "stdio") {
			const args = [];
			for (const [index, arg] of config.args.entries()) args.push(await resolve(arg, `args[${index}]`));
			return {
				...config,
				command: await resolve(config.command, "command"),
				args,
				env: await record(config.env, "env"),
				cwd: await resolve(config.cwd, "cwd")
			};
		}
		return {
			...config,
			url: await resolve(config.url, "url"),
			headers: await record(config.headers, "headers")
		};
	}
};
//#endregion
//#region lib/types/tools.js
/**
* Tool bridge: discovers MCP tools, registers them on the harness ToolRuntime
* under deterministic server-qualified public names, and handles re-sync when
* the server's tool list changes.
*
* Naming contract (see the mcp-client Agent Note "Naming invariants"): every MCP tool
* has the stable identity `(serverName, rawName)`; the model-facing public name
* is `mcp__<serverName>__<rawName>`, normalized to the DeepSeek function-name
* constraints. The raw name is only ever sent on the wire (`tools/call`); the
* public name is never parsed to recover it.
*
* @module
*/
/** Registration failed after the prior generation was removed and partial registrations were rolled back. */
var McpToolRegistrationError = class extends McpLocalError {};
/**
* DeepSeek function-name contract: at most 64 characters. Wire-protocol
* constant, not configuration.
*/
const MAX_PUBLIC_NAME_LENGTH = 64;
/** DeepSeek function-name contract: only `[A-Za-z0-9_-]` is allowed. */
const INVALID_NAME_CHARS = /[^A-Za-z0-9_-]/g;
/** Hex chars of the SHA-256 identity hash appended on lossy normalization. */
const HASH_LENGTH = 12;
/** Raster formats supported by the durable attachment vocabulary. */
const IMAGE_MEDIA_TYPES = [
	"image/png",
	"image/jpeg",
	"image/webp",
	"image/gif"
];
/** Canonical RFC 4648 base64, excluding whitespace and URL-safe aliases. */
const CANONICAL_BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
/**
* Derive the model-facing public name for one MCP tool.
*
* Deterministic pure function of `(serverName, rawName)`: the clean case is
* `mcp__<serverName>__<rawName>` verbatim. When character replacement or
* truncation to the DeepSeek function-name contract (64 chars,
* `[A-Za-z0-9_-]`) changes the name, a 12-hex-char SHA-256 hash of the
* identity is appended so distinct MCP identities never collapse into the
* same public name.
*
* @param serverName - Stable local namespace from plugin config.
* @param rawName - The MCP server's own tool name.
* @returns The globally unique, model-facing ToolRuntime name.
*/
function publicToolName(serverName, rawName) {
	const joined = `mcp__${serverName}__${rawName}`;
	const normalized = joined.replace(INVALID_NAME_CHARS, "_");
	if (normalized === joined && normalized.length <= MAX_PUBLIC_NAME_LENGTH) return normalized;
	const hash = createHash("sha256").update(`${serverName}\0${rawName}`).digest("hex").slice(0, HASH_LENGTH);
	return `${normalized.slice(0, MAX_PUBLIC_NAME_LENGTH - HASH_LENGTH - 1)}_${hash}`;
}
/**
* Sync the MCP server's tool list into the harness ToolRuntime.
*
* Two phases keep the swap safe:
*
* 1. Fetch: let the SDK aggregate `tools/list` and build the full next
*    generation of `ToolDefinition`s under public names. Any failure here
*    (network error or duplicate raw name) rejects
*    and leaves the previous generation registered untouched.
* 2. Swap: dispose the previous generation, register the new one. A registry
*    conflict here can only mean a foreign registration squats on this
*    server's `mcp__<serverName>__` namespace — the partial generation is
*    rolled back (zero tools from this server), logged, and rejected so the
*    supervisor closes the failed connection generation.
*
* @param client - Connected MCP Client instance used to list and call tools.
* @param ctx - Cordis context providing the `tools` service for registration.
* @param opts - server namespace, call timeout, generation signal, and optional error formatter.
* @param previous - Disposer map from the prior sync generation; disposed
*   during the swap phase (only after the fetch phase succeeded).
* @param eligible - Judged again at the commit point, after the listing
*   settled: a sync whose generation was abandoned meanwhile (startup budget
*   elapsed, connection closed) publishes nothing and leaves `previous`
*   untouched.
* @returns A map of registered public tool names to their unregister
*   disposers — the exact set of live registrations owned by this server.
*/
async function syncTools(client, ctx, opts, previous, eligible = () => true) {
	const definitions = /* @__PURE__ */ new Map();
	const response = client.getServerCapabilities()?.tools === void 0 ? { tools: [] } : await client.listTools(void 0, {
		cacheMode: "refresh",
		signal: opts.signal
	});
	for (const tool of response.tools) {
		const publicName = publicToolName(opts.serverName, tool.name);
		if (definitions.has(publicName)) throw new Error(`mcp-client(${opts.serverName}): server listed tool "${tool.name}" more than once — invalid tool list`);
		definitions.set(publicName, createMcpToolDefinition(ctx, {
			name: publicName,
			rawName: tool.name,
			description: tool.description ?? "",
			inputSchema: tool.inputSchema,
			outputSchema: tool.outputSchema,
			taskRequired: tool.execution?.taskSupport === "required",
			call: async (args, execution) => {
				try {
					const result = await client.callTool({
						name: tool.name,
						arguments: args
					}, {
						signal: AbortSignal.any([execution.signal, opts.signal]),
						timeout: opts.toolCallTimeoutMs,
						toolDefinition: tool
					});
					if (result.isError !== true || opts.redact === void 0) return result;
					return {
						isError: true,
						content: [{
							type: "text",
							text: opts.redact(extractText(result.content, tool.name))
						}]
					};
				} catch (error) {
					if (opts.redact === void 0) throw error;
					throw new Error(opts.redact(describeError(error)));
				}
			}
		}));
	}
	if (!eligible()) return previous;
	for (const dispose of previous.values()) dispose();
	const disposers = /* @__PURE__ */ new Map();
	try {
		for (const [publicName, definition] of definitions) disposers.set(publicName, ctx.tools.register(definition));
	} catch (error) {
		for (const dispose of disposers.values()) dispose();
		ctx.logger.error(`mcp-client(${opts.serverName}): tool registration failed, no tools registered: ${String(error)}`);
		throw new McpToolRegistrationError("MCP tool registration failed", { cause: error });
	}
	return disposers;
}
/** Keep a supported advertised schema; unsupported MCP vocabulary falls back to JsonValue. */
function supportedOutputSchema(candidate) {
	if (candidate === void 0) return void 0;
	try {
		assertSupportedJsonSchema(candidate);
		return candidate;
	} catch {
		return;
	}
}
/**
* Adapt an upstream MCP tool to canonical values and durable image content.
* Registration, provider lifetime, deadlines, and transport belong to the caller.
* @param ctx - plugin context carrying optional attachment and model services.
* @param options - upstream tool fields and its raw-result callback.
* @returns the unregistered ToolRuntime definition.
*/
function createMcpToolDefinition(ctx, options) {
	const { name, rawName, description, inputSchema } = options;
	const projections = /* @__PURE__ */ new WeakMap();
	return {
		name,
		description,
		parameters: inputSchema,
		output: createOutput(rawName, supportedOutputSchema(options.outputSchema)),
		execute: createExecutor(ctx, options, projections),
		projectContent(exec, result) {
			const projection = projections.get(exec);
			if (projection === void 0) return void 0;
			projections.delete(exec);
			if (result.isError) return void 0;
			if (!isDeepStrictEqual(result.value, projection.value)) return void 0;
			if (!isDeepStrictEqual(result.content, projection.fallback)) return void 0;
			return projection.content;
		}
	};
}
/** Build the canonical result schema and existing Native text projection. */
function createOutput(rawName, structuredSchema) {
	return {
		schema: {
			type: "object",
			properties: {
				content: {
					type: "array",
					items: {}
				},
				structuredContent: structuredSchema ?? {}
			},
			required: structuredSchema === void 0 ? ["content"] : ["content", "structuredContent"],
			additionalProperties: false
		},
		render(_args, value) {
			return [{
				type: "text",
				text: extractText(value.content, rawName)
			}];
		}
	};
}
/**
* Invoke the caller-owned raw-result callback and prepare canonical content.
* MCP isError results reject before image storage so ToolRuntime records failure.
*/
function createExecutor(ctx, options, projections) {
	const { rawName, taskRequired } = options;
	return async (args, exec) => {
		if (taskRequired) throw new Error(`Tool "${rawName}" requires task-based execution, which this bridge does not support`);
		const argsObj = typeof args === "object" && args !== null ? args : {};
		const parsed = specTypeSchemas.CallToolResult["~standard"].validate(await options.call(argsObj, exec));
		if (parsed.issues !== void 0) throw new Error(`Tool "${rawName}" returned an invalid MCP result: ${parsed.issues.map((issue) => issue.message).join("; ")}`);
		const result = parsed.value;
		const content = result.content;
		const text = extractText(content, rawName);
		if (result.isError === true) throw new Error(text);
		const value = {
			content,
			...result.structuredContent !== void 0 ? { structuredContent: result.structuredContent } : {}
		};
		if (containsImage(content)) {
			const fallback = [{
				type: "text",
				text: extractText(content, rawName)
			}];
			const projected = await prepareImageProjection(ctx, exec, content, rawName);
			projections.set(exec, {
				value,
				fallback,
				content: projected
			});
		}
		return value;
	};
}
/** Whether an untrusted MCP content array contains a declared image block. */
function containsImage(content) {
	return content.some((value) => isRecord(value) && value.type === "image");
}
/** Narrow one JSON value to a string-keyed object. */
function isRecord(value) {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
/** Narrow a declared MIME string to the durable image vocabulary. */
function isImageMediaType(value) {
	return IMAGE_MEDIA_TYPES.includes(value);
}
/** Decode one projected image without accepting base64 aliases. */
function decodeImage(block) {
	if (!isImageMediaType(block.mimeType)) throw new Error("the declared media type is not PNG, JPEG, WebP, or GIF");
	if (!CANONICAL_BASE64.test(block.data)) throw new Error("the image data is not canonical base64");
	const data = Buffer.from(block.data, "base64");
	if (data.toString("base64") !== block.data) throw new Error("the image data is not canonical base64");
	return {
		data,
		mediaType: block.mimeType
	};
}
/**
* Resolve the active model route and durable store for an image-bearing result.
* @param ctx - plugin context with optional services.
* @param exec - exact tool execution whose agent supplies the latest route.
* @returns the attachment store after exact positive image-capability proof.
*/
async function resolveImageAdmission(ctx, exec) {
	const attachments = ctx.get("attachments");
	if (attachments === void 0) throw new Error("no attachment store is mounted");
	const routed = exec.agent?.session.requestHeader()?.config;
	const provider = routed?.provider ?? exec.agent?.options.provider;
	const model = routed?.model ?? exec.agent?.options.model;
	const llm = ctx.get("llm");
	if (provider === void 0 || model === void 0 || llm === void 0) throw new Error("the current model route could not be resolved");
	let info;
	try {
		info = await llm.resolveModelInfo(provider, model, exec.signal);
	} catch {
		throw new Error("the current model route could not be verified");
	}
	if (info.inputModalities === void 0 || !info.inputModalities.includes("image")) throw new Error(`model "${model}" does not declare image input`);
	if (exec.signal.aborted) throw new Error("the tool call was canceled before image storage");
	return attachments;
}
/** Stable diagnostic text for an image block that was not admitted. */
function imageDiagnostic(block, reason) {
	return `[image unavailable: ${block.mimeType ?? "unknown media type"}; ${reason}; raw image data remains available to programmatic callers]`;
}
/**
* Decode, preflight, and durably save one MCP result's ordered image batch.
* Any refusal projects every image as text while retaining the canonical raw
* value for programmatic callers.
*/
async function prepareImageProjection(ctx, exec, content, toolName) {
	const decoded = [];
	const validationErrors = /* @__PURE__ */ new Map();
	const imageIndexes = [];
	for (const [index, value] of content.entries()) {
		if (!isRecord(value) || value.type !== "image") continue;
		imageIndexes.push(index);
		try {
			decoded.push(decodeImage(value));
		} catch (error) {
			validationErrors.set(index, error.message);
		}
	}
	if (validationErrors.size > 0) return projectContent(content, toolName, (block, index) => ({
		type: "text",
		text: imageDiagnostic(block, validationErrors.get(index) ?? "another image in the same result was invalid")
	}));
	let attachments;
	try {
		attachments = await resolveImageAdmission(ctx, exec);
	} catch (error) {
		const reason = error.message;
		return projectContent(content, toolName, (block) => ({
			type: "text",
			text: imageDiagnostic(block, reason)
		}));
	}
	try {
		const refs = await attachments.saveImages(decoded);
		const byIndex = new Map(imageIndexes.map((index, offset) => [index, refs[offset]]));
		return projectContent(content, toolName, (_block, index) => ({
			type: "image",
			attachment: byIndex.get(index)
		}));
	} catch (error) {
		const reason = isImageAdmissionError(error) ? `image admission rejected the result: ${error.message}` : "durable image storage rejected the result";
		return projectContent(content, toolName, (block) => ({
			type: "text",
			text: imageDiagnostic(block, reason)
		}));
	}
}
/**
* Extract text from an MCP content array into a single string.
* - text blocks: join with '\n'
* - image/audio/resource blocks: replaced with a placeholder
*
* Policy-owned canonical-value replacements may omit fields required on the MCP wire.
*/
function extractText(mcpContent, toolName) {
	return projectContent(mcpContent, toolName).map((block) => block.text).join("\n");
}
/**
* Project ordered MCP blocks into the core content vocabulary.
* Text-like runs are newline-coalesced; admitted images split those runs at
* their original position.
*/
function projectContent(mcpContent, toolName, image = (block) => ({
	type: "text",
	text: imageDiagnostic(block, "this result was not admitted to durable model context")
})) {
	const projected = [];
	const text = [];
	const flushText = () => {
		if (text.length === 0) return;
		projected.push({
			type: "text",
			text: text.splice(0).join("\n")
		});
	};
	for (const [index, value] of mcpContent.entries()) {
		if (!isRecord(value)) {
			text.push("[unsupported MCP content block: expected an object]");
			continue;
		}
		const block = value;
		switch (block.type) {
			case "text":
				if (block.text !== void 0) text.push(block.text);
				break;
			case "image":
				flushText();
				projected.push(image(block, index));
				break;
			case "resource_link":
				if (block.name === void 0 || block.uri === void 0) text.push("[resource link unavailable: the MCP block is missing its name or URI]");
				else text.push(`Resource link: ${block.name} (${block.uri})`);
				break;
			case "audio":
				text.push(`[audio result unsupported: ${block.mimeType ?? "unknown media type"}; raw audio data remains available to programmatic callers]`);
				break;
			case "resource":
				text.push("[embedded resource unsupported; raw resource data remains available to programmatic callers]");
				break;
			default: text.push(`[unsupported MCP content type: ${block.type}]`);
		}
	}
	flushText();
	return projected.length > 0 ? projected : [{
		type: "text",
		text: `(${toolName} returned no model-visible content)`
	}];
}
//#endregion
//#region lib/types/connection.js
/**
* Connection supervisor: owns the MCP client/transport generations for one
* plugin instance, keeps the harness tool registry in sync with the live
* generation, and — when the connection drops — restarts the configured
* server with bounded exponential backoff.
*
* One outage shares one attempt budget (`maxAttempts` consecutive failed
* attempts, delays doubling from `initialDelayMs` up to `maxDelayMs`). A
* connection that stays up past the stability window closes the outage, so
* the next disconnect starts a fresh budget while a crash-looping server —
* even one whose connects briefly succeed — still exhausts the cap instead of
* restarting forever. Exhaustion unregisters the server's tools and stops;
* a changed credential binding or a fresh instance resets that budget.
*
* @module
*/
/** Defaults shared by the Config schema and {@link resolveReconnectPolicy}. */
const RECONNECT_DEFAULTS = Object.freeze({
	enabled: true,
	initialDelayMs: 500,
	maxDelayMs: 3e4,
	maxAttempts: 10
});
/** Default UTF-8 byte limit for attributed server instructions. */
const DEFAULT_MAX_INSTRUCTION_BYTES = 32768;
/**
* The one explicit resolve step from raw reconnect config to the policy the
* supervisor runs. Programmatic construction may bypass Schemastery
* normalization, so every default and bound is re-judged here — misconfiguration
* fails the plugin instance at load.
*
* @param config - Raw `reconnect` config; omission uses the defaults.
* @param path - Diagnostic prefix naming the config location in thrown messages.
* @returns The frozen resolved policy.
*/
function resolveReconnectPolicy(config, path) {
	if (config !== void 0) {
		for (const key of Object.keys(config)) if (!Object.hasOwn(RECONNECT_DEFAULTS, key)) throw new Error(`${path}.${key} is not a reconnect option`);
	}
	const enabled = config?.enabled ?? RECONNECT_DEFAULTS.enabled;
	const initialDelayMs = config?.initialDelayMs ?? RECONNECT_DEFAULTS.initialDelayMs;
	const maxDelayMs = config?.maxDelayMs ?? RECONNECT_DEFAULTS.maxDelayMs;
	const maxAttempts = config?.maxAttempts ?? RECONNECT_DEFAULTS.maxAttempts;
	if (!Number.isFinite(initialDelayMs) || initialDelayMs <= 0 || initialDelayMs > MAX_TIMER_DELAY_MS) throw new Error(`${path}.initialDelayMs must be a positive finite number no greater than ${MAX_TIMER_DELAY_MS}`);
	if (!Number.isFinite(maxDelayMs) || maxDelayMs <= 0 || maxDelayMs > MAX_TIMER_DELAY_MS) throw new Error(`${path}.maxDelayMs must be a positive finite number no greater than ${MAX_TIMER_DELAY_MS}`);
	if (initialDelayMs > maxDelayMs) throw new Error(`${path}.initialDelayMs must be less than or equal to maxDelayMs`);
	if (!Number.isInteger(maxAttempts) || maxAttempts < 1) throw new Error(`${path}.maxAttempts must be a positive integer`);
	return Object.freeze({
		enabled,
		initialDelayMs,
		maxDelayMs,
		maxAttempts
	});
}
/**
* Start the supervised connection for one MCP server and keep it alive per
* the reconnect policy. The first attempt starts in a microtask; the caller
* must register disposal before yielding.
*
* @param ctx - Cordis context providing the `tools` registry and logger.
* @param config - Resolved plugin config selecting the transport and server identity.
* @param policy - Resolved reconnect policy from {@link resolveReconnectPolicy}.
* @param status - Instance status cell this supervisor publishes transitions to.
* @returns Handle with a `ready` promise for startup-await and a `dispose` for teardown.
*/
function startConnection(ctx, config, policy, status) {
	const label = `mcp-client(${config.serverName})`;
	const binding = new McpTemplateBinding(ctx, config);
	const diagnostic = (error, operation) => binding.diagnostic(error, operation);
	const lifetime = new AbortController();
	const readiness = Promise.withResolvers();
	let current;
	let bindingValue;
	let disposed = false;
	let serverInstructions = "";
	let disposers = /* @__PURE__ */ new Map();
	let syncChain = Promise.resolve();
	let settling = Promise.resolve();
	let reconnectTimer;
	let failedAttempts = 0;
	let closureFailure;
	let checkingCredentials;
	let credentialsDirty = false;
	const isCurrent = (record) => !disposed && current === record && !record.wire.lifetime.signal.aborted;
	function stopTimers() {
		clearTimeout(reconnectTimer);
		reconnectTimer = void 0;
	}
	function clearTools() {
		for (const dispose of disposers.values()) try {
			dispose();
		} catch (error) {
			closureFailure = {
				...status.closure(),
				cleanupError: diagnostic(error, "tool cleanup")
			};
			status.recordClosure(closureFailure);
			ctx.logger.error(`${label}: tool cleanup failed: ${diagnostic(error, "tool cleanup")}`);
		}
		disposers = /* @__PURE__ */ new Map();
		serverInstructions = "";
	}
	async function close(record) {
		const result = await record.wire.close();
		if (!result.closeConfirmed || result.cleanupError !== void 0) closureFailure ??= result;
		status.recordClosure(closureFailure ?? result);
		if (closureFailure !== void 0 && !disposed) {
			const error = new Error(closureFailure.cleanupError);
			status.update({
				state: "failed",
				reason: "unclosed-generation",
				lastError: error.message
			});
			readiness.resolve({ error });
		}
		return closureFailure === void 0;
	}
	function enqueueSync(record) {
		const run = syncChain.then(async () => {
			if (!isCurrent(record)) return;
			await record.wire.run(async (signal) => {
				disposers = await syncTools(record.wire.client, ctx, {
					serverName: config.serverName,
					toolCallTimeoutMs: config.toolCallTimeoutMs,
					signal,
					...config.expandTemplates ? { redact: (text) => binding.redact(text) } : {}
				}, disposers, () => isCurrent(record));
			});
		});
		syncChain = run.then(() => void 0, () => void 0);
		return run;
	}
	function scheduleReconnect(error, connectedAt) {
		if (disposed || closureFailure !== void 0) return;
		if (!policy.enabled) {
			const message = connectedAt !== void 0 ? "connection lost and reconnect is disabled — registered tools will fail until an HMR reload or Host restart" : "connection failed and reconnect is disabled — no tools were registered; reload the plugin or restart the Host to connect";
			status.update({
				state: "failed",
				reason: "reconnect-disabled",
				lastError: error
			});
			ctx.logger.error(`${label}: ${message}`);
			return;
		}
		if (connectedAt !== void 0 && Date.now() - connectedAt >= policy.maxDelayMs) failedAttempts = 0;
		failedAttempts++;
		if (failedAttempts > policy.maxAttempts) {
			syncChain = syncChain.then(clearTools);
			status.update({
				state: "failed",
				reason: "gave-up",
				lastError: error
			});
			ctx.logger.error(`${label}: giving up after ${policy.maxAttempts} consecutive failed reconnect attempts — tools unregistered; reload the plugin or restart the Host to reconnect`);
			return;
		}
		const delayMs = Math.min(policy.maxDelayMs, policy.initialDelayMs * 2 ** (failedAttempts - 1));
		const action = connectedAt !== void 0 ? "connection lost; reconnecting" : "connection failed; retrying";
		ctx.logger.warn(`${label}: ${action} in ${delayMs}ms (attempt ${failedAttempts}/${policy.maxAttempts})`);
		status.update({
			state: "reconnecting",
			attempt: failedAttempts,
			maxAttempts: policy.maxAttempts,
			nextRetryAtMs: Date.now() + delayMs,
			lastError: error
		});
		reconnectTimer = setTimeout(() => {
			reconnectTimer = void 0;
			settling = connectGeneration();
		}, delayMs);
		reconnectTimer.unref();
	}
	function lost(record, error) {
		if (!isCurrent(record)) return;
		current = void 0;
		settling = close(record).then((confirmed) => {
			readiness.resolve({ error: new Error(error) });
			if (confirmed) scheduleReconnect(error, record.connectedAt);
		});
	}
	function isStopped() {
		return disposed || closureFailure !== void 0;
	}
	function requestCredentialRefresh() {
		if (isStopped()) return;
		credentialsDirty = true;
		checkingCredentials ??= (async () => {
			while (credentialsDirty && !isStopped()) {
				credentialsDirty = false;
				let next;
				try {
					next = await binding.resolve(lifetime.signal);
				} catch (error) {
					if (!(error instanceof MissingTemplateValue)) throw error;
				}
				if (disposed || bindingValue !== void 0 && deepEqualJson(next, bindingValue.config)) continue;
				stopTimers();
				const record = current;
				const attempt = settling;
				current = void 0;
				if (record !== void 0) await close(record);
				await attempt;
				await syncChain;
				stopTimers();
				clearTools();
				if (isStopped()) {
					readiness.resolve({ error: /* @__PURE__ */ new Error("MCP credential refresh cleanup is incomplete") });
					return;
				}
				failedAttempts = 0;
				bindingValue = next === void 0 ? {} : { config: next };
				settling = connectGeneration();
			}
		})().catch((error) => {
			if (!disposed) ctx.logger.error(`${label}: credential refresh failed: ${diagnostic(error, "credential refresh")}`);
		}).finally(() => {
			checkingCredentials = void 0;
			if (credentialsDirty) requestCredentialRefresh();
		});
	}
	if (binding.refs.size > 0) ctx.on("credentials/reference-updated", (ref) => {
		if (binding.refs.has(String(ref))) requestCredentialRefresh();
	});
	async function connectGeneration() {
		if (isStopped()) return;
		const client = new Client({
			name: "dsh-mcp-client",
			version: "0.0.1"
		}, {
			capabilities: {},
			versionNegotiation: { mode: "auto" },
			listChanged: { tools: {
				autoRefresh: false,
				debounceMs: 0,
				onChanged: () => {
					refreshTools();
				}
			} }
		});
		const record = { wire: new McpGeneration(client, config.closeTimeoutMs, (error) => diagnostic(error, "cleanup")) };
		current = record;
		status.opened();
		status.update({
			state: "connecting",
			attempt: failedAttempts
		});
		client.onclose = () => {
			if (record.connectedAt !== void 0) lost(record, "connection closed");
		};
		async function refreshTools() {
			if (!isCurrent(record)) return;
			ctx.logger.info(`${label}: tool list changed, re-syncing`);
			try {
				await enqueueSync(record);
				if (isCurrent(record) && record.connectedAt !== void 0) status.update({
					state: "connected",
					toolCount: disposers.size,
					connectedAt: record.connectedAt
				});
			} catch (error) {
				if (error instanceof McpToolRegistrationError) lost(record, diagnostic(error, "tool synchronization"));
				else if (isCurrent(record)) ctx.logger.error(`${label}: tool re-sync failed: ${diagnostic(error, "tool synchronization")}`);
			}
		}
		try {
			const startedAt = Date.now();
			const remaining = () => config.startupTimeoutMs === void 0 ? void 0 : config.startupTimeoutMs - (Date.now() - startedAt);
			const resolved = await record.wire.run((signal) => binding.resolve(signal), remaining());
			bindingValue = { config: resolved };
			await record.wire.connect(resolved, remaining());
			if (!isCurrent(record)) return;
			const text = client.getInstructions()?.trimEnd() ?? "";
			const instructions = text ? `### MCP server: ${config.serverName}\n\n${text}` : "";
			const maxInstructionBytes = config.maxInstructionBytes ?? 32768;
			if (Buffer.byteLength(instructions) > maxInstructionBytes) throw new McpLocalError(`${label}: server instructions exceed maxInstructionBytes (${maxInstructionBytes})`);
			const sync = enqueueSync(record);
			const budget = remaining();
			await (budget === void 0 ? sync : withDeadline(sync, budget, `${label}: startup timed out`));
			if (!isCurrent(record)) return;
			record.wire.assertOpen();
			serverInstructions = instructions;
			record.connectedAt = Date.now();
			status.update({
				state: "connected",
				toolCount: disposers.size,
				connectedAt: record.connectedAt
			});
			readiness.resolve({});
			if (failedAttempts > 0) ctx.logger.info(`${label}: reconnected and re-synced tools (attempt ${failedAttempts}/${policy.maxAttempts})`);
		} catch (error) {
			if (current !== record) return;
			current = void 0;
			const confirmed = await close(record);
			readiness.resolve({ error: config.expandTemplates ? new Error(diagnostic(error, "startup")) : error });
			if (disposed || !confirmed) return;
			if (error instanceof MissingTemplateValue) {
				bindingValue = {};
				status.update({
					state: "waiting-credentials",
					lastError: diagnostic(error, "credential resolution")
				});
				return;
			}
			ctx.logger.warn(`${label}: connection attempt failed: ${diagnostic(error, "startup")}`);
			scheduleReconnect(diagnostic(error, "startup"));
		}
	}
	settling = Promise.resolve().then(connectGeneration);
	return {
		ready: readiness.promise,
		instructions: () => serverInstructions,
		resources: { async request(request, exec) {
			const record = current;
			if (record?.connectedAt === void 0) throw new Error(`${label}: server is disconnected`);
			try {
				return await record.wire.run(async (signal) => {
					const options = {
						signal: AbortSignal.any([signal, exec.signal]),
						timeout: config.toolCallTimeoutMs
					};
					const generation = record.wire.client;
					const listable = generation.getServerCapabilities()?.resources !== void 0;
					switch (request.method) {
						case "resources/list": return listable ? await generation.listResources(request.cursor === void 0 ? void 0 : { cursor: request.cursor }, options) : { resources: [] };
						case "resources/templates/list": return listable ? await generation.listResourceTemplates(request.cursor === void 0 ? void 0 : { cursor: request.cursor }, options) : { resourceTemplates: [] };
						case "resources/read": return await generation.readResource({ uri: request.uri }, options);
						/* v8 ignore next -- exhaustive over the closed resource request union */
						default: return assertNever(request);
					}
				});
			} catch (error) {
				if (config.expandTemplates) throw new Error(diagnostic(error, request.method));
				throw error;
			}
		} },
		async dispose() {
			disposed = true;
			readiness.resolve({ error: /* @__PURE__ */ new Error("MCP client disposed") });
			lifetime.abort(/* @__PURE__ */ new Error("MCP client disposed"));
			stopTimers();
			const record = current;
			current = void 0;
			if (record !== void 0) await close(record);
			try {
				await withDeadline(Promise.all([
					settling,
					syncChain,
					checkingCredentials
				]), config.closeTimeoutMs, "MCP cleanup did not settle");
			} catch (error) {
				closureFailure ??= {
					...status.closure(),
					cleanupError: diagnostic(error, "cleanup")
				};
			}
			clearTools();
			const result = closureFailure ?? status.closure();
			status.recordClosure(result);
			status.update({
				state: "closed",
				result
			});
			if (!result.closeConfirmed || result.cleanupError !== void 0) ctx.logger.error(`${label}: cleanup incomplete`);
		}
	};
}
//#endregion
//#region lib/types/server-context.js
/**
* Publish connection-owned MCP resources and literal server instructions.
*
* @module @deepseek-ai/dsh-mcp-client
*/
/**
* Contribute server context to the services enabled by this composition.
* @param ctx - server plugin's registration scope and effect owner.
* @param server - configured server identity.
* @param connection - live resource operations and successful instruction snapshot.
*/
function registerServerContext(ctx, server, connection) {
	ctx.inject(["mcpResources"], (inner) => {
		inner.mcpResources.register(server, connection.resources);
	});
	ctx.inject(["systemPrompt"], (inner) => {
		inner.systemPrompt.section({
			name: `mcp:${server}`,
			order: inner.systemPrompt.getSectionOrder("MCP_SERVERS"),
			interpolate: false,
			text: () => connection.instructions()
		});
	});
}
//#endregion
//#region lib/types/status.js
/** Writable status cell retained across supervisors on the same Fiber. */
var McpStatusCell = class {
	warn;
	value = {
		state: "connecting",
		attempt: 0
	};
	closeResult = { closeConfirmed: true };
	closure() {
		return this.closeResult;
	}
	/** Mark the next generation's closure as unconfirmed. */
	opened() {
		this.closeResult = { closeConfirmed: false };
	}
	/**
	* Record cleanup without publishing a connection-status transition.
	* @param result - independently observed transport and local cleanup outcome.
	*/
	recordClosure(result) {
		this.closeResult = result;
	}
	/** Remove existing observers without discarding the readable status or cleanup result. */
	detach() {
		this.listeners.clear();
	}
	listeners = /* @__PURE__ */ new Set();
	/**
	* @param warn - contained-listener diagnostic sink (the owning plugin's logger).
	*/
	constructor(warn) {
		this.warn = warn;
	}
	/**
	* Associate observer diagnostics with the Fiber's current supervisor.
	* @param warn - contained-listener diagnostic sink.
	*/
	setWarningSink(warn) {
		this.warn = warn;
	}
	/**
	* The current status.
	* @returns the latest published status.
	*/
	current() {
		return this.value;
	}
	/**
	* Observe status transitions; see {@link McpConnectionStatusHandle.onChange}.
	* @param callback - invoked with each published status.
	* @returns the disposer removing this observer.
	*/
	onChange(callback) {
		this.listeners.add(callback);
		return () => void this.listeners.delete(callback);
	}
	/**
	* Publish the next status and notify observers. A throwing observer is
	* contained so a broken consumer cannot wedge the supervisor's transition.
	* @param next - the status to publish.
	*/
	update(next) {
		this.value = next;
		for (const listener of [...this.listeners]) try {
			const returned = listener(next);
			if (typeof returned?.then === "function") Promise.resolve(returned).catch((error) => {
				this.warn("an onChange status observer failed", error);
			});
		} catch (error) {
			this.warn("an onChange status observer failed", error);
		}
	}
};
const cellsByFiber = /* @__PURE__ */ new WeakMap();
/**
* Create or reuse a Fiber's status cell without discarding prior cleanup results.
* @param fiber - the actual plugin Fiber owning the supervisor.
* @param warn - diagnostic sink for observer failures.
* @returns the stable writable cell for this Fiber.
*/
function registerStatusCell(fiber, warn) {
	let cell = cellsByFiber.get(fiber);
	if (cell === void 0) {
		cell = new McpStatusCell(warn);
		cellsByFiber.set(fiber, cell);
	} else cell.setWarningSink(warn);
	return cell;
}
/**
* Look up status through an actual Fiber or the handle returned by `ctx.plugin()`.
* @param fiber - plugin Fiber or its thenable handle.
* @returns the retained cell, or `undefined` before the MCP instance registers one.
*/
function connectionStatusIn(fiber) {
	return cellsByFiber.get(fiber.ctx.fiber);
}
//#endregion
//#region lib/types/probe.js
/**
* One-shot MCP server probe: connect, read the server identity, list tools,
* close. Nothing registers on `ctx.tools`, no `serverName` is reserved, and
* nothing persists — the probe answers a configuration surface's
* "test this server" without touching the live instance set. Expected
* failures return a stage-classified value rather than throwing, so a wire
* caller renders the outcome without exception mapping.
*
* @module
*/
/** Default whole-probe budget (ms): interactive surfaces need a bounded answer. */
const DEFAULT_PROBE_TIMEOUT_MS = 1e4;
/** Listing page cap: a cursor loop or firehose server ends as truncation, not a hang. */
const PROBE_MAX_PAGES = 16;
/** Tool-name cap on the returned list. */
const PROBE_MAX_TOOL_NAMES = 512;
function resolveProbeOptions(options) {
	const timeoutMs = options.timeoutMs ?? DEFAULT_PROBE_TIMEOUT_MS;
	const maxPages = options.maxPages ?? PROBE_MAX_PAGES;
	const maxToolNames = options.maxToolNames ?? PROBE_MAX_TOOL_NAMES;
	if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new TypeError("probeServer: timeoutMs must be a positive finite number");
	if (!Number.isSafeInteger(maxPages) || maxPages < 1) throw new TypeError("probeServer: maxPages must be a positive safe integer");
	if (!Number.isSafeInteger(maxToolNames) || maxToolNames < 1) throw new TypeError("probeServer: maxToolNames must be a positive safe integer");
	return {
		timeoutMs,
		maxPages,
		maxToolNames,
		signal: options.signal
	};
}
/**
* Probe a server without registering tools or reserving its namespace.
* @param ctx - context providing template sources when expansion is enabled.
* @param config - transport configuration; its startup budget does not bound the probe.
* @param options - whole-probe budget, listing caps, and cancellation signal.
* @returns the connect/list outcome and independent cleanup result after bounded closure.
* @throws for invalid options, malformed templates, or a missing required credentials provider.
*/
async function probeServer(ctx, config, options = {}) {
	const spec = resolveProbeOptions(options);
	const startedAt = Date.now();
	const remaining = () => spec.timeoutMs - (Date.now() - startedAt);
	const binding = new McpTemplateBinding(ctx, config);
	const client = new Client({
		name: "dsh-mcp-probe",
		version: "0.0.1"
	}, {
		capabilities: {},
		versionNegotiation: { mode: "auto" }
	});
	const generation = new McpGeneration(client, config.closeTimeoutMs, (error) => binding.diagnostic(error, "cleanup"));
	const abort = () => {
		generation.close();
	};
	spec.signal?.addEventListener("abort", abort, { once: true });
	if (spec.signal?.aborted) abort();
	let stage = "connect";
	let result;
	try {
		const resolved = await generation.run((signal) => binding.resolve(signal), remaining());
		await generation.connect(resolved, remaining());
		const identity = client.getServerVersion();
		const serverInfo = identity === void 0 ? void 0 : {
			name: identity.name,
			version: identity.version
		};
		stage = "list";
		const toolNames = [];
		let truncated = false;
		let cursor;
		if (client.getServerCapabilities()?.tools !== void 0) for (let page = 0; page < spec.maxPages; page++) {
			const response = await generation.run((signal) => client.request({
				method: "tools/list",
				...cursor === void 0 ? {} : { params: { cursor } }
			}, { signal }), remaining());
			for (const tool of response.tools) {
				if (toolNames.length >= spec.maxToolNames) {
					truncated = true;
					break;
				}
				toolNames.push(tool.name);
			}
			cursor = response.nextCursor;
			if (cursor === void 0 || truncated) break;
			if (page === spec.maxPages - 1) truncated = true;
		}
		result = {
			ok: true,
			...serverInfo === void 0 ? {} : { serverInfo },
			toolNames,
			truncated,
			durationMs: Date.now() - startedAt
		};
	} catch (error) {
		result = {
			ok: false,
			stage,
			message: spec.signal?.aborted ? "probe aborted" : binding.diagnostic(error, stage),
			durationMs: Date.now() - startedAt
		};
	} finally {
		spec.signal?.removeEventListener("abort", abort);
	}
	return {
		...result,
		...await generation.close()
	};
}
//#endregion
//#region lib/types/index.js
/**
* MCP client bridge plugin: connects to an external MCP server and registers
* its tools on `ctx.tools` under server-qualified public names
* (`mcp__<serverName>__<rawName>`). Each plugin instance connects to one MCP
* server; load multiple instances in `cordis.yml` for multiple servers.
*
* Namespace plugin (named exports, no default export). Lifecycle is
* effect-scoped: disposal disconnects from the server, unregisters all tools,
* and releases the `serverName` namespace reservation. HMR hot-swaps by
* disposing the old instance and creating a new one; identical `serverName`
* reproduces identical public tool names.
*
* @module @deepseek-ai/dsh-mcp-client
*/
/** Cordis plugin name used by loader diagnostics. */
const name = "mcp-client";
/** Services required by this plugin. */
const inject = ["tools"];
/** Default timeout for individual MCP tool calls and resource requests (ms). */
const DEFAULT_TOOL_CALL_TIMEOUT_MS = 6e4;
/** Default budget for transport closure and local cleanup (ms). */
const DEFAULT_CLOSE_TIMEOUT_MS = 5e3;
/** Valid `serverName`, kept below the public tool-name budget. */
const SERVER_NAME_PATTERN = /^[A-Za-z0-9_-]{1,32}$/;
/**
* Live `serverName` reservations per registration scope. Agent-scoped MCP
* servers may reuse a namespace in another Agent, while global instances and
* duplicates inside one Agent remain mutually exclusive.
*/
const activeServerNames = /* @__PURE__ */ new WeakMap();
/**
* Read an MCP instance's stable status handle. Reloading the same Fiber keeps
* the handle; final disposal detaches observers and leaves its last state readable.
* @param fiber - the plugin Fiber or the handle returned by `ctx.plugin()`.
* @returns the status handle, or `undefined` before the instance registers one.
*/
function connectionStatusOf(fiber) {
	return connectionStatusIn(fiber);
}
/**
* Reconnect-policy section schema, exported so a manager composing mcp-client
* instances (`dsh-mcp-manager`) validates the same fields, defaults, and
* bounds without restating them.
*/
const ReconnectSchema = z.object({
	enabled: z.boolean().default(RECONNECT_DEFAULTS.enabled),
	initialDelayMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(RECONNECT_DEFAULTS.initialDelayMs),
	maxDelayMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(RECONNECT_DEFAULTS.maxDelayMs),
	maxAttempts: z.number().step(1).min(1).max(Number.MAX_SAFE_INTEGER).default(RECONNECT_DEFAULTS.maxAttempts)
});
const Config = z.union([z.object({
	transport: z.const("stdio"),
	serverName: z.string().required().pattern(SERVER_NAME_PATTERN),
	command: z.string().required(),
	args: z.array(String).default([]),
	env: z.dict(String).default({}),
	cwd: z.string().default(""),
	toolCallTimeoutMs: z.number().default(DEFAULT_TOOL_CALL_TIMEOUT_MS),
	startupTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS),
	closeTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(DEFAULT_CLOSE_TIMEOUT_MS),
	expandTemplates: z.boolean().default(false),
	failOnStartupError: z.boolean().default(false),
	maxInstructionBytes: z.number().step(1).min(1).default(DEFAULT_MAX_INSTRUCTION_BYTES),
	reconnect: ReconnectSchema
}), z.object({
	transport: z.const("streamable-http"),
	serverName: z.string().required().pattern(SERVER_NAME_PATTERN),
	url: z.string().required(),
	headers: z.dict(String).default({}),
	toolCallTimeoutMs: z.number().default(DEFAULT_TOOL_CALL_TIMEOUT_MS),
	startupTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS),
	closeTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(DEFAULT_CLOSE_TIMEOUT_MS),
	expandTemplates: z.boolean().default(false),
	failOnStartupError: z.boolean().default(false),
	maxInstructionBytes: z.number().step(1).min(1).default(DEFAULT_MAX_INSTRUCTION_BYTES),
	reconnect: ReconnectSchema
})]);
/**
* Connect one MCP server and publish its initial tool generation before activation.
* This entry remains explicitly `async`: Cordis treats a prototype-bearing
* ordinary function as a constructor, whose returned Promise is not startup work.
* @param ctx - plugin context carrying the tool registry.
* @param config - resolved transport and server namespace configuration.
* @returns startup readiness after connection and initial tool discovery settle.
*/
async function apply(ctx, config) {
	const reconnect = resolveReconnectPolicy(config.reconnect, `mcp-client(${config.serverName}): reconnect`);
	let status;
	ctx.effect(() => {
		const owner = scopeOf(ctx) ?? ctx.root;
		let names = activeServerNames.get(owner);
		if (!names) {
			names = /* @__PURE__ */ new Set();
			activeServerNames.set(owner, names);
		}
		if (names.has(config.serverName)) throw new Error(`mcp-client: serverName "${config.serverName}" is already in use by another mcp-client instance — pick a unique serverName in cordis.yml`);
		names.add(config.serverName);
		status = registerStatusCell(ctx.fiber, (message, error) => {
			ctx.logger.warn(`mcp-client(${config.serverName}): ${message}`);
			if (!config.expandTemplates) ctx.logger.warn(error);
		});
		return () => {
			if (ctx.fiber.uid === null) status.detach();
			names.delete(config.serverName);
		};
	}, "mcp-client.serverName");
	if (!status.closure().closeConfirmed || status.closure().cleanupError !== void 0) throw new Error(`mcp-client(${config.serverName}): previous connection cleanup is incomplete; restart the Host`);
	const connection = startConnection(ctx, config, reconnect, status);
	let stopping;
	const dispose = () => stopping ??= connection.dispose();
	ctx.on("internal/plugin", (fiber) => {
		if (fiber !== ctx.fiber || fiber.uid !== null) return;
		return dispose();
	}, { global: true });
	ctx.effect(() => dispose, "mcp-client.connection");
	registerServerContext(ctx, config.serverName, connection);
	const outcome = await connection.ready;
	if (outcome.error !== void 0 && config.failOnStartupError) throw new Error(`mcp-client(${config.serverName}): initial connection or tool synchronization failed`, { cause: outcome.error });
}
//#endregion
export { Config, ReconnectSchema, SERVER_NAME_PATTERN, apply, assertTemplateSyntax, connectionStatusOf, createMcpToolDefinition, inject, name, probeServer, resolveReconnectPolicy };
