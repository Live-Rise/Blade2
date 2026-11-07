import z from "@deepseek-ai/schemastery";
import { z as z$1 } from "zod";
import { createUserMessage } from "@deepseek-ai/dsh-llm";
import { SessionSeq } from "@deepseek-ai/dsh-session";
import { Duration, canonicalizeTimeZone } from "@deepseek-ai/dsh-util-time";
import { assertNever } from "@deepseek-ai/dsh-util-values";
//#region lib/types/request-zone.js
/** Browser-zone facts from committed or proposed user-rpc messages and model-facing policy text. */
/**
* Read the raw zone reported by a user-rpc source; validation belongs to request preparation.
* @param message - user message; only user sources with a string rpcId and clientTimeZone contribute.
* @returns the reported zone, or undefined when no user-rpc zone was reported.
*/
function reportedBrowserTimeZone(message) {
	const source = message.source;
	return source.kind === "user" && "rpcId" in source && typeof source.rpcId === "string" && "clientTimeZone" in source && typeof source.clientTimeZone === "string" ? source.clientTimeZone : void 0;
}
/** Validate a reported zone at request preparation, preserving the platform refusal. */
function browserTimeZone(value) {
	const resolved = canonicalizeTimeZone(value);
	if (resolved.kind === "accepted") {
		if (resolved.timeZone !== value) throw new TypeError(`browser time zone must be canonical: ${JSON.stringify(value)}`);
		return value;
	}
	switch (resolved.reason) {
		case "unsupported": throw new TypeError(`browser time zone is unsupported: ${JSON.stringify(value)}`, { cause: resolved.cause });
		case "blank":
		case "padded":
		case "not_area_location": throw new TypeError(`browser time zone must be canonical UTC or IANA Area/Location: ${JSON.stringify(value)}`);
		/* v8 ignore next 2 -- Intl answers a canonical Area/Location for every name it accepts. */
		case "not_canonical_shape": throw new TypeError(`browser time zone must be canonical: ${JSON.stringify(value)}`);
		/* v8 ignore next 2 -- TimeZoneRejection is closed; this arm only makes adding a reason a compile error. */
		default: assertNever(resolved.reason, "TimeZoneRejection");
	}
}
/**
* Derive the unique, mixed, or missing browser zone for the selected request messages.
* @param messages - proposed messages, or entered plus proposed messages selected by the legacy reader.
* @param enteredTimeZones - reported zones from committed messages, before the proposed messages.
* @returns Sorted, duplicate-free browser-zone facts.
* @throws TypeError when a user-rpc source carries an invalid or noncanonical zone.
*/
function deriveBrowserTimeZoneContext(messages, enteredTimeZones = []) {
	const reported = messages.flatMap((message) => {
		const timeZone = reportedBrowserTimeZone(message);
		return timeZone === void 0 ? [] : [timeZone];
	});
	const timeZones = [...new Set([...enteredTimeZones, ...reported].map(browserTimeZone))].sort();
	const [timeZone, ...remaining] = timeZones;
	if (timeZone === void 0) return { kind: "missing" };
	if (remaining.length === 0) return {
		kind: "resolved",
		timeZone
	};
	return {
		kind: "mixed",
		timeZones
	};
}
/**
* Render the model instruction for one browser-zone context.
* @param context - browser-zone facts selected for this request.
* @returns One durable policy line.
*/
function renderBrowserTimeZoneContext(context) {
	switch (context.kind) {
		case "resolved": return `Browser time zone for this request: ${context.timeZone}. Interpret otherwise-unqualified dates and times in this zone.`;
		case "mixed": return `Browser time zone for this request: mixed ${JSON.stringify(context.timeZones)}. Ask the user to clarify otherwise-unqualified dates and times.`;
		case "missing": return "Browser time zone for this request: unavailable. Ask the user to clarify otherwise-unqualified dates and times.";
		/* v8 ignore next 2 -- the closed BrowserTimeZoneContext union is exhausted above. */
		default: return assertNever(context, "BrowserTimeZoneContext");
	}
}
//#endregion
//#region lib/types/timestamp.js
/** ISO-shaped time-context timestamp formatting shared by production and replay validation. */
/**
* Create the exact formatter used by durable time-context readings.
* @param timeZone - Explicit display zone, or `undefined` for the process fallback.
* @returns A formatter with stable numeric local fields and long numeric offset.
*/
function createTimestampFormatter(timeZone) {
	return new Intl.DateTimeFormat("en-US", {
		...timeZone === void 0 ? {} : { timeZone },
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		hourCycle: "h23",
		timeZoneName: "longOffset"
	});
}
/**
* Format an epoch millisecond value as an ISO-shaped timestamp with offset and IANA zone.
* @param now - Epoch milliseconds to display.
* @param formatter - Formatter created for `timeZone`.
* @param timeZone - Canonical zone label carried in brackets.
* @returns The durable timestamp text.
*/
function formatTimestamp(now, formatter, timeZone) {
	const parts = Object.fromEntries(formatter.formatToParts(now).map((part) => [part.type, part.value]));
	const offset = parts.timeZoneName.replace(/^GMT$/, "GMT+00:00").slice(3);
	return `${parts["year"]}-${parts["month"]}-${parts["day"]}T${parts["hour"]}:${parts["minute"]}:${parts["second"]}${offset}[${timeZone}]`;
}
//#endregion
//#region lib/types/index.js
/**
* Opt-in request clock context. Eligible steps add durable,
* source-attributed time readings to the request history.
*
* @module @deepseek-ai/dsh-time-context
*/
/** Cordis plugin name used by loader diagnostics. */
const name = "time-context";
const timeContextStateSchema = z$1.object({
	/** Time of the latest model-visible event (user/assistant message, tool result), or null. */
	lastMessageTime: z$1.number().nullable(),
	/** Time of this plugin's latest durable injection, or null. */
	lastInjectionTime: z$1.number().nullable(),
	/** Latest injection time in the open turn, or null before that turn receives one. */
	lastTurnInjectionTime: z$1.number().nullable(),
	/** Latest entered turn; null before any turn/start record. */
	browserTurn: z$1.number().nullable(),
	/** Distinct reported zones in encounter order; validation remains request-time. */
	browserTimeZones: z$1.array(z$1.string())
});
/** The agent registry that owns pre-step processing. */
const inject = ["agents", "sessionProjections"];
/** Schemastery validation for {@link Config}. */
const Config = z.object({
	timeZone: z.string(),
	refreshIntervalMs: z.number()
});
/**
* Format a non-negative elapsed millisecond count as compact whole-second units.
*
* A backward wall-clock reading is a property of the clock rather than of the
* span, so it clamps to zero here; the shared span type then supplies the whole
* parts, and this package keeps the wording.
*/
function formatDuration(elapsedMs) {
	const { days, hours, minutes, seconds } = Duration.fromMilliseconds(Math.max(0, elapsedMs)).decompose();
	const parts = [];
	if (days > 0) parts.push(`${days}d`);
	if (hours > 0) parts.push(`${hours}h`);
	if (minutes > 0) parts.push(`${minutes}m`);
	parts.push(`${seconds}s`);
	return parts.join(" ");
}
/** Preserve the original scan for typed pre-step callers requesting a nonlatest turn. */
function requestMessages(agent, turn, proposed) {
	const entered = [];
	for (let seq = agent.session.seq - 1; seq >= 0; seq -= 1) {
		const event = agent.session.eventAt(SessionSeq(seq));
		if (event?.type === "turn/start" && event.data.turn === turn) return [...entered.reverse(), ...proposed];
		if (event?.type === "user/message") entered.push(event.data);
	}
	return [...proposed];
}
function renderText(now, turn, step, previous, formatter, timeZone, browserContext) {
	const elapsed = previous === void 0 ? "unavailable" : formatDuration(now - previous);
	const baseline = step === 1 ? "model-visible message" : "step context";
	const browserText = renderBrowserTimeZoneContext(browserContext);
	return `Time sampled while preparing turn ${turn}, step ${step}: ${formatTimestamp(now, formatter, timeZone)}\n${browserText}\nElapsed since the preceding ${baseline}: ${elapsed}.`;
}
/** Reject refresh intervals that cannot represent an exact elapsed-millisecond threshold. */
function validateRefreshInterval(refreshIntervalMs) {
	if (!Number.isSafeInteger(refreshIntervalMs) || refreshIntervalMs < 0) throw new TypeError(`time-context: refreshIntervalMs must be a non-negative safe integer, got ${String(refreshIntervalMs)}`);
}
/**
* Register a prepended pre-step listener for the lifetime of `ctx`.
* Steps admitted during activation await the projection registration; a
* registration failure or owner disposal prevents the request from proceeding.
* @param ctx - plugin context; the listener is disposed with it.
* @param config - time zone and durable refresh scheduling configuration.
* @throws when the refresh interval is invalid or the configured or process time zone cannot be resolved.
* @returns when the plugin is ready.
*/
async function apply(ctx, config) {
	const timeZone = config.timeZone;
	const refreshIntervalMs = config.refreshIntervalMs ?? 6e5;
	validateRefreshInterval(refreshIntervalMs);
	let fallbackFormatter;
	try {
		fallbackFormatter = createTimestampFormatter(timeZone);
	} catch (error) {
		const message = timeZone === void 0 ? "time-context: failed to resolve the system time zone" : `time-context: invalid IANA timeZone ${JSON.stringify(timeZone)}`;
		throw new Error(message, { cause: error });
	}
	const fallbackTimeZone = fallbackFormatter.resolvedOptions().timeZone;
	const formatters = new Map([[fallbackTimeZone, fallbackFormatter]]);
	/** Resolve and cache one request-local timestamp formatter. */
	const formatterFor = (selectedTimeZone) => {
		const existing = formatters.get(selectedTimeZone);
		if (existing !== void 0) return existing;
		const created = createTimestampFormatter(selectedTimeZone);
		formatters.set(selectedTimeZone, created);
		return created;
	};
	const projectionReady = ctx.sessionProjections.register({
		key: "timeContext",
		stateVersion: 3,
		stateSchema: timeContextStateSchema,
		init: () => ({
			lastMessageTime: null,
			lastInjectionTime: null,
			lastTurnInjectionTime: null,
			browserTurn: null,
			browserTimeZones: []
		}),
		apply: (state, event) => {
			if (event.type === "turn/start") return {
				...state,
				lastTurnInjectionTime: null,
				browserTurn: event.data.turn,
				browserTimeZones: []
			};
			if (event.type === "turn/end") return state.lastTurnInjectionTime === null ? state : {
				...state,
				lastTurnInjectionTime: null
			};
			if (event.type === "user/message") {
				const injected = event.data.source.kind === name;
				let withMessage = state.lastMessageTime === event.time ? state : {
					...state,
					lastMessageTime: event.time
				};
				const timeZone = reportedBrowserTimeZone(event.data);
				if (state.browserTurn !== null && timeZone !== void 0 && !state.browserTimeZones.includes(timeZone)) withMessage = {
					...withMessage,
					browserTimeZones: [...state.browserTimeZones, timeZone]
				};
				if (!injected) return withMessage;
				return {
					...withMessage,
					lastInjectionTime: event.time,
					lastTurnInjectionTime: event.time
				};
			}
			if (event.type === "assistant/message" || event.type === "tool/result") return state.lastMessageTime === event.time ? state : {
				...state,
				lastMessageTime: event.time
			};
			return state;
		}
	});
	ctx.on("agent/pre-step", async ({ agent, turn, step, signal }, next) => {
		const timeContextProjectionHandle = await projectionReady;
		ctx.fiber.assertActive();
		const decision = await next();
		if (decision.kind === "reject" || signal.aborted) return decision;
		const now = Date.now();
		const state = timeContextProjectionHandle.stateOf(agent.session);
		if (refreshIntervalMs > 0) {
			const lastInjection = state.lastInjectionTime;
			if (lastInjection != null && now >= lastInjection && now - lastInjection < refreshIntervalMs) return decision;
		}
		/* v8 ignore next 3 -- every later step follows a recorded injection in the same turn */
		const previous = step === 1 ? state.lastMessageTime ?? void 0 : state.lastTurnInjectionTime ?? void 0;
		const browser = state.browserTurn === turn ? deriveBrowserTimeZoneContext(decision.messages, state.browserTimeZones) : deriveBrowserTimeZoneContext(requestMessages(agent, turn, decision.messages));
		const selectedTimeZone = browser.kind === "resolved" ? browser.timeZone : fallbackTimeZone;
		const text = renderText(now, turn, step, previous, formatterFor(selectedTimeZone), selectedTimeZone, browser);
		return {
			...decision,
			messages: [...decision.messages, createUserMessage({
				content: [{
					type: "text",
					text
				}],
				source: {
					kind: name,
					form: "snapshot",
					sections: [{
						name,
						text
					}]
				}
			})]
		};
	}, { prepend: true });
	await projectionReady;
}
//#endregion
export { Config, apply, inject, name };
