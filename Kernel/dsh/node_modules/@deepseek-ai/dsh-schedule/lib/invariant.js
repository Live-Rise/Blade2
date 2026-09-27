import "@js-temporal/polyfill";
import { SessionLogOffset } from "@deepseek-ai/dsh-session";
const MIN_FOUR_DIGIT_YEAR_MS = Date.parse("0001-01-01T00:00:00.000Z");
const MAX_FOUR_DIGIT_YEAR_MS = Date.parse("9999-12-31T23:59:59.999Z");
const UTC_INSTANT = /^(?!0000)\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d\.\d{3}Z$/;
new RegExp(String.raw`^(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})` + String.raw`T(?<hour>\d{2}):(?<minute>\d{2}):(?<second>\d{2})` + String.raw`(?:\.(?<fraction>\d{1,3}))?(?<zone>Z|(?<sign>[+-])` + String.raw`(?<offsetHour>\d{2}):(?<offsetMinute>\d{2}))$`);
/** Error from malformed or transition-invalid durable Schedule data. */
var ScheduleLogError = class extends Error {
	/** Stable machine-readable error code. */
	code = "corrupt_schedule_log";
	/**
	* Construct a durable-log failure.
	* @param message - Package-specific violated invariant.
	*/
	constructor(message) {
		super(message);
		this.name = "ScheduleLogError";
	}
};
/**
* Brand a raw session-local id without changing its runtime value.
* @param value - Raw session-local id.
* @returns The same string with the Schedule brand.
*/
function ScheduleId(value) {
	return value;
}
/** Whether an unknown value is a non-array object. */
function isRecord(value) {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
/** Require exactly the named durable object keys. */
function hasExactKeys(value, expected) {
	const allowed = new Set(expected);
	return expected.every((key) => key in value) && Object.keys(value).every((key) => allowed.has(key));
}
/** Require the named durable keys while admitting further optional members. */
function hasExactKeysWithOptional(value, required, optional) {
	const allowed = new Set([...required, ...optional]);
	return required.every((key) => key in value) && Object.keys(value).every((key) => allowed.has(key));
}
/** Stable diagnostic for a title that is missing or empty after trimming. */
const REQUIRED_TITLE_MESSAGE = "title is required and must be non-empty after trimming.";
/**
* Validate one required stored title at the durable boundary.
*
* Only records written after titles became required are read: a missing,
* blank-after-trim, untrimmed, or over-long stored title is invalid, and no name
* is derived from the instruction.
* @param value - Untrusted durable title field.
* @returns The stored title; an invalid title throws ScheduleLogError.
*/
function decodeStoredTitle(value) {
	if (typeof value !== "string" || value.trim().length === 0) throw new ScheduleLogError(REQUIRED_TITLE_MESSAGE);
	if (value.length > 120) throw new ScheduleLogError(`title must be at most 120 characters`);
	if (value.trim() !== value) throw new ScheduleLogError("title must be a trimmed string");
	return value;
}
/**
* Decode the stored title of one historical `schedule/change` create record.
*
* A version-1 event written before names existed has no `title` member, so the
* key set is required without it and the absent member decodes as undefined. A
* present title stays subject to the canonical stored form.
* @param value - Untrusted durable record already known to be an object.
* @param expected - Exact keys the record may carry; `title` is optional within them.
* @param message - Diagnostic naming the record's keys.
* @returns The stored title, or undefined when the historical record predates it.
*/
function decodeHistoricalRecordTitle(value, expected, message) {
	if (!hasExactKeysWithOptional(value, expected.filter((key) => key !== "title"), ["title"])) throw new ScheduleLogError(message);
	return value["title"] === void 0 ? void 0 : decodeStoredTitle(value["title"]);
}
/** Validate one stable session-local id at the durable boundary. */
function decodeId(value) {
	if (typeof value !== "string" || value.length === 0 || value.trim() !== value) throw new ScheduleLogError("schedule id must be a non-empty string without surrounding whitespace");
	return ScheduleId(value);
}
/** Validate one canonical four-digit-year UTC instant. */
function decodeInstant(value) {
	if (typeof value !== "string" || !UTC_INSTANT.test(value)) throw new ScheduleLogError("scheduledAt must be a canonical four-digit-year RFC 3339 UTC instant");
	const epoch = Date.parse(value);
	if (!Number.isFinite(epoch) || new Date(epoch).toISOString() !== value) throw new ScheduleLogError("scheduledAt is not a real UTC calendar instant");
	return value;
}
/** Decode the exact v1 after record shape. */
function decodeAfterRecord(value) {
	/* v8 ignore next -- decodeLegacyScheduleRecord rejects a non-object before it dispatches here. */
	if (!isRecord(value)) throw new ScheduleLogError("after schedule must be an object");
	const title = decodeHistoricalRecordTitle(value, [
		"id",
		"kind",
		"title",
		"prompt",
		"afterSeconds",
		"scheduledAt"
	], "after schedule must contain exactly id, kind, title, prompt, afterSeconds, and scheduledAt");
	const prompt = value["prompt"];
	if (typeof prompt !== "string" || prompt.length === 0 || prompt.trim() !== prompt) throw new ScheduleLogError("after prompt must be non-empty and already trimmed");
	const afterSeconds = value["afterSeconds"];
	if (!Number.isSafeInteger(afterSeconds) || afterSeconds <= 0) throw new ScheduleLogError("afterSeconds must be a positive safe integer");
	return Object.freeze({
		id: decodeId(value["id"]),
		kind: "after",
		...title === void 0 ? {} : { title },
		prompt,
		afterSeconds,
		scheduledAt: decodeInstant(value["scheduledAt"])
	});
}
/** Decode the exact v1 absolute one-shot record shape. */
function decodeAtRecord(value) {
	/* v8 ignore next -- decodeLegacyScheduleRecord rejects a non-object before it dispatches here. */
	if (!isRecord(value)) throw new ScheduleLogError("at schedule must be an object");
	const title = decodeHistoricalRecordTitle(value, [
		"id",
		"kind",
		"title",
		"prompt",
		"scheduledAt"
	], "at schedule must contain exactly id, kind, title, prompt, and scheduledAt");
	const prompt = value["prompt"];
	if (typeof prompt !== "string" || prompt.length === 0 || prompt.trim() !== prompt) throw new ScheduleLogError("at prompt must be non-empty and already trimmed");
	return Object.freeze({
		id: decodeId(value["id"]),
		kind: "at",
		...title === void 0 ? {} : { title },
		prompt,
		scheduledAt: decodeInstant(value["scheduledAt"])
	});
}
/** Decode the exact v1 fixed-rate record shape. */
function decodeEveryRecord(value) {
	/* v8 ignore next -- decodeLegacyScheduleRecord rejects a non-object before it dispatches here. */
	if (!isRecord(value)) throw new ScheduleLogError("every schedule must be an object");
	const title = decodeHistoricalRecordTitle(value, [
		"id",
		"kind",
		"title",
		"prompt",
		"everySeconds",
		"scheduledAt"
	], "every schedule must contain exactly id, kind, title, prompt, everySeconds, and scheduledAt");
	const prompt = value["prompt"];
	if (typeof prompt !== "string" || prompt.length === 0 || prompt.trim() !== prompt) throw new ScheduleLogError("every prompt must be non-empty and already trimmed");
	const everySeconds = value["everySeconds"];
	const interval = typeof everySeconds === "number" ? everySeconds * 1e3 : NaN;
	if (!Number.isSafeInteger(everySeconds) || everySeconds < 60 || !Number.isSafeInteger(interval)) throw new ScheduleLogError(`everySeconds must be a safe integer of at least 60`);
	return Object.freeze({
		id: decodeId(value["id"]),
		kind: "every",
		...title === void 0 ? {} : { title },
		prompt,
		everySeconds,
		scheduledAt: decodeInstant(value["scheduledAt"])
	});
}
/**
* Decode only the variants admitted by historical version-1 Session events.
*
* A record written before titles existed decodes without that member, so the
* fold keeps reading a log that the current creation path could not write.
* @param value - Untrusted durable JSON record from a Session event.
* @returns Detached frozen record, without a title when the event omitted it.
*/
function decodeLegacyScheduleRecord(value) {
	if (!isRecord(value)) throw new ScheduleLogError("schedule record must be an object");
	switch (value["kind"]) {
		case "after": return decodeAfterRecord(value);
		case "at": return decodeAtRecord(value);
		case "every": return decodeEveryRecord(value);
		default: throw new ScheduleLogError("v1 schedule kind must be \"after\", \"at\", or \"every\"");
	}
}
/**
* Decode one strict version-1 `schedule/change` payload.
* @param value - Untrusted durable JSON value.
* @returns Detached, frozen Schedule change.
*/
function decodeScheduleChange(value) {
	if (!isRecord(value)) throw new ScheduleLogError("schedule/change payload must be an object");
	if (value["version"] !== 1) throw new ScheduleLogError("schedule/change version must be 1");
	switch (value["operation"]) {
		case "create":
			if (!hasExactKeys(value, [
				"version",
				"operation",
				"schedule"
			])) throw new ScheduleLogError("schedule create must contain exactly version, operation, and schedule");
			return Object.freeze({
				version: 1,
				operation: "create",
				schedule: decodeLegacyScheduleRecord(value["schedule"])
			});
		case "delete":
			if (!hasExactKeys(value, [
				"version",
				"operation",
				"id"
			])) throw new ScheduleLogError("schedule delete must contain exactly version, operation, and id");
			return Object.freeze({
				version: 1,
				operation: "delete",
				id: decodeId(value["id"])
			});
		case "dispatch":
			if (hasExactKeys(value, [
				"version",
				"operation",
				"id"
			])) return Object.freeze({
				version: 1,
				operation: "dispatch",
				id: decodeId(value["id"])
			});
			if (hasExactKeys(value, [
				"version",
				"operation",
				"id",
				"acceptedAt"
			])) return Object.freeze({
				version: 1,
				operation: "dispatch",
				id: decodeId(value["id"]),
				acceptedAt: decodeInstant(value["acceptedAt"])
			});
			throw new ScheduleLogError("schedule dispatch must contain id and optional acceptedAt only");
		default: throw new ScheduleLogError("schedule/change operation must be create, delete, or dispatch");
	}
}
/**
* Resolve one fixed-rate decision without enumerating missed occurrences.
* @param record - Active record whose target is the earliest unaccepted occurrence.
* @param acceptedAt - Wall-clock decision time in epoch milliseconds.
* @returns The latest due occurrence and first strictly future target, if representable.
*/
function resolveEveryOccurrence(record, acceptedAt) {
	const target = Date.parse(record.scheduledAt);
	const interval = record.everySeconds * 1e3;
	if (!Number.isSafeInteger(acceptedAt) || acceptedAt < MIN_FOUR_DIGIT_YEAR_MS || acceptedAt > MAX_FOUR_DIGIT_YEAR_MS) throw new ScheduleLogError("every acceptedAt must be a representable four-digit-year instant");
	if (!Number.isSafeInteger(interval) || interval <= 0) throw new ScheduleLogError("every interval milliseconds must be a positive safe integer");
	if (acceptedAt < target) throw new ScheduleLogError("every dispatch cannot precede the active scheduledAt");
	const occurrence = target + Math.floor((acceptedAt - target) / interval) * interval;
	/* v8 ignore next -- bounded operands and a quotient-derived product stay safe. */
	if (!Number.isSafeInteger(occurrence) || occurrence < target || occurrence > acceptedAt) throw new ScheduleLogError("every occurrence arithmetic must stay within the accepted interval");
	const occurrenceAt = new Date(occurrence).toISOString();
	const next = occurrence + interval;
	if (!Number.isSafeInteger(next) || next > MAX_FOUR_DIGIT_YEAR_MS) return Object.freeze({ occurrenceAt });
	return Object.freeze({
		occurrenceAt,
		nextScheduledAt: new Date(next).toISOString()
	});
}
/** Apply one decoded dispatch to its exact active record. */
function dispatchedRecord(record, change) {
	const hasAcceptedAt = "acceptedAt" in change;
	if (record.kind !== "every") {
		if (hasAcceptedAt) throw new ScheduleLogError("one-shot dispatch must not contain acceptedAt");
		return;
	}
	if (!hasAcceptedAt) throw new ScheduleLogError("every dispatch must contain acceptedAt");
	const occurrence = resolveEveryOccurrence(record, Date.parse(change.acceptedAt));
	return occurrence.nextScheduledAt === void 0 ? void 0 : Object.freeze({
		...record,
		scheduledAt: occurrence.nextScheduledAt
	});
}
/**
* Apply already-decoded Schedule changes to one complete fold value.
*
* The transition authority for full-log replay. One mutable Map/Set pair spans
* the whole batch; the returned arrays are materialized and frozen once.
* @param folded - complete active records and used-id history before the changes.
* @param changes - strictly decoded durable mutations in log order.
* @returns the complete fold value after every mutation.
*/
function applyScheduleChanges(folded, changes) {
	const active = new Map(folded.active.map((record) => [record.id, record]));
	const seen = new Set(folded.seenIds);
	for (const change of changes) switch (change.operation) {
		case "create":
			if (seen.has(change.schedule.id)) throw new ScheduleLogError(`schedule id ${JSON.stringify(change.schedule.id)} was reused`);
			seen.add(change.schedule.id);
			active.set(change.schedule.id, change.schedule);
			break;
		case "delete":
			if (!active.delete(change.id)) throw new ScheduleLogError(`schedule delete targets inactive id ${JSON.stringify(change.id)}`);
			break;
		case "dispatch": {
			const record = active.get(change.id);
			if (record === void 0) throw new ScheduleLogError(`schedule dispatch targets inactive id ${JSON.stringify(change.id)}`);
			const next = dispatchedRecord(record, change);
			if (next === void 0) active.delete(change.id);
			else active.set(change.id, next);
			break;
		}
		/* v8 ignore next 3 -- decodeScheduleChange returns a closed operation union. */
		default: throw new ScheduleLogError(`unknown decoded schedule change ${String(change)}`);
	}
	return Object.freeze({
		active: Object.freeze([...active.values()]),
		seenIds: Object.freeze([...seen])
	});
}
/**
* Fold the package-owned stream after the durable fork seed boundary.
* @param events - Complete ordered session log or candidate-extended log.
* @param inheritedEventCount - Inherited prefix length excluded from child ownership.
* @returns Active records and all previously used ids.
*/
function foldScheduleEvents(events, inheritedEventCount = SessionLogOffset(0)) {
	if (!Number.isSafeInteger(inheritedEventCount) || inheritedEventCount < 0 || inheritedEventCount > events.length) throw new ScheduleLogError("schedule inheritedEventCount must be within the supplied event log");
	const initial = Object.freeze({
		active: Object.freeze([]),
		seenIds: Object.freeze([])
	});
	const changes = function* () {
		for (const event of events.slice(inheritedEventCount)) if (event.type === "schedule/change") yield decodeScheduleChange(event.data);
	};
	return applyScheduleChanges(initial, changes());
}
//#endregion
//#region lib/types/invariant.js
/**
* Package-owned strict Schedule stream invariant.
* @module @deepseek-ai/dsh-schedule/invariant
*/
const PACKAGE_NAME = "@deepseek-ai/dsh-schedule";
/** Cordis invariant-companion plugin name. */
const name = "schedule-invariant";
/** Service required before reserving this package's invariant ownership. */
const inject = ["invariants"];
/** Validate a complete exact-session stream under its fork suffix policy. */
function validate(events, fail) {
	try {
		foldScheduleEvents(events);
	} catch (error) {
		/* v8 ignore next -- foldScheduleEvents normalizes every rejected stream to ScheduleLogError. */
		if (!(error instanceof ScheduleLogError)) throw error;
		fail(error.message);
	}
}
/**
* Install replay and pre-append validation for the owned event stream.
*
* The durable task table is not asserted here: `scheduleDomain` validates every stored
* record through its table schema when the domain opens, and its only writer stores each
* task under `task.record.id`, so a check of the write payload would observe the same value
* the writer had just built.
*/
const install = Object.assign((ctx, fail) => {
	for (const session of ctx.sessions.list()) validate(session.ownEvents(), fail);
	ctx.on("session/created", (session) => {
		validate(session.ownEvents(), fail);
	}, { global: true });
	ctx.on("internal/dispatch", (_mode, eventName, args) => {
		if (eventName !== "session/event") return;
		const [session, event] = args;
		if (event.type !== "schedule/change") return;
		validate([...session.ownEvents(), event], fail);
	}, { global: true });
}, { inject: ["sessions"] });
/**
* Register the package-owned invariant companion.
* @param ctx - Cordis context carrying the invariant registry.
* @returns Exact registration disposer after child setup succeeds.
*/
const apply = (ctx) => Promise.resolve(ctx.invariants.register(PACKAGE_NAME, install));
//#endregion
export { apply, inject, name };
