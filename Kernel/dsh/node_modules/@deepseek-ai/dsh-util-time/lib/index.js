import { Temporal } from "@js-temporal/polyfill";
import rrulePackage from "rrule";
//#region lib/types/instant.js
/**
* Strict instant and local-calendar vocabulary for the boundaries that accept a
* wall-clock selector.
*
* Every entry that reads untrusted text answers a discriminated result instead
* of throwing: the library carries no failure vocabulary, and each caller maps
* a reason onto its own domain code and diagnostic. `formatFourDigitYearInstant`
* and `isFourDigitYearInstant` never throw, while `resolveLocalInstant` raises a
* `RangeError` when a caller violates its precondition — an unknown zone, or
* calendar fields naming a value the calendar does not have. Local wall-clock
* resolution uses the maintained `Temporal` implementation, so no caller owns
* offset sampling or DST rules.
* @module @deepseek-ai/dsh-util-time
*/
/** Inclusive lower epoch-millisecond bound of a four-digit-year RFC 3339 UTC instant. */
const MIN_FOUR_DIGIT_YEAR_MS = Date.parse("0001-01-01T00:00:00.000Z");
/** Inclusive upper epoch-millisecond bound of a four-digit-year RFC 3339 UTC instant. */
const MAX_FOUR_DIGIT_YEAR_MS = Date.parse("9999-12-31T23:59:59.999Z");
/** Canonical four-digit-year RFC 3339 UTC instant with mandatory milliseconds. */
const UTC_INSTANT = /^(?!0000)\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d\.\d{3}Z$/;
/** RFC 3339 instant with mandatory date, time, and explicit zone; fraction is optional. */
const OFFSET_INSTANT = new RegExp(String.raw`^(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})` + String.raw`T(?<hour>\d{2}):(?<minute>\d{2}):(?<second>\d{2})` + String.raw`(?:\.(?<fraction>\d{1,3}))?(?<zone>Z|(?<sign>[+-])` + String.raw`(?<offsetHour>\d{2}):(?<offsetMinute>\d{2}))$`);
/** Four-digit ISO calendar date. */
const LOCAL_DATE = /^(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})$/;
/** Local wall-clock time with optional one-to-three digit fractional seconds. */
const LOCAL_TIME = /^(?<hour>\d{2}):(?<minute>\d{2}):(?<second>\d{2})(?:\.(?<fraction>\d{1,3}))?$/;
/** Strict browser-zone profile: UTC or an IANA Area/Location-style identifier. */
const IANA_TIME_ZONE = /^[A-Za-z][A-Za-z0-9_+.-]*(?:\/[A-Za-z0-9_+.-]+)+$/;
/** Reject with one stable reason. */
function rejected(reason) {
	return Object.freeze({
		kind: "rejected",
		reason
	});
}
/** Read one required named regular-expression group as a number. */
function groupNumber(groups, name) {
	const value = groups[name];
	/* v8 ignore next -- a successful fixed regex always provides every requested group. */
	if (value === void 0) throw new TypeError(`missing required regex group ${name}`);
	return Number(value);
}
/** Normalize an optional one-to-three digit fractional second to milliseconds. */
function fractionalMilliseconds(value) {
	return value === void 0 ? 0 : Number(value.padEnd(3, "0"));
}
/** Whether an integral epoch is inside the four-digit-year range. */
function inFourDigitYearRange(epoch) {
	return Number.isSafeInteger(epoch) && epoch >= MIN_FOUR_DIGIT_YEAR_MS && epoch <= MAX_FOUR_DIGIT_YEAR_MS;
}
/**
* Build one plain date-time while refusing a date the calendar does not have.
*
* Every field is already range-checked by the caller, so the only refusal left
* is a non-existent calendar date such as February 30.
* @param time - Calendar fields with every numeric field in range.
* @returns The plain date-time, or `undefined` when the date does not exist.
*/
function plainDateTime(time) {
	try {
		return Temporal.PlainDateTime.from(time, { overflow: "reject" });
	} catch (error) {
		/* v8 ignore next -- a rejected overflow answers a RangeError, so the false path is unreachable. */
		if (error instanceof RangeError) return void 0;
		/* v8 ignore next -- reached only when Temporal throws something other than a RangeError. */
		throw error;
	}
}
/**
* Read one local calendar selector strictly, without consulting a process zone.
*
* The date and time texts are matched separately, so a malformed spelling, a
* non-existent calendar date such as February 30, and an out-of-range clock
* field answer distinct reasons.
* @param date - Candidate `YYYY-MM-DD` date text.
* @param time - Candidate `HH:mm:ss` time text with optional 1-3 fractional digits.
* @returns The parsed calendar fields, or the reason they are unusable.
*/
function parseLocalCalendar(date, time) {
	const dateMatch = LOCAL_DATE.exec(date);
	const timeMatch = LOCAL_TIME.exec(time);
	const dateGroups = dateMatch?.groups;
	const timeGroups = timeMatch?.groups;
	if (dateGroups === void 0 || timeGroups === void 0) return rejected("malformed");
	const parts = {
		year: groupNumber(dateGroups, "year"),
		month: groupNumber(dateGroups, "month"),
		day: groupNumber(dateGroups, "day"),
		hour: groupNumber(timeGroups, "hour"),
		minute: groupNumber(timeGroups, "minute"),
		second: groupNumber(timeGroups, "second"),
		millisecond: fractionalMilliseconds(timeGroups["fraction"])
	};
	if (parts.year === 0 || parts.hour > 23 || parts.minute > 59 || parts.second > 59) return rejected("out_of_range");
	if (plainDateTime(parts) === void 0) return rejected("not_a_calendar_date");
	return Object.freeze({
		kind: "accepted",
		time: parts
	});
}
/**
* Parse one RFC 3339 instant whose numeric offset is part of the input.
*
* A bare `Z` and an explicit numeric offset are both accepted, and the offset is
* subtracted from the local wall-clock value. A negative zero offset, an offset
* whose hour or minute is out of range, and a non-existent calendar date are refused.
* @param value - Candidate instant text.
* @returns The resolved epoch milliseconds, or the reason the text is unusable.
*/
function parseOffsetInstant(value) {
	const groups = OFFSET_INSTANT.exec(value)?.groups;
	if (groups === void 0) return rejected("not_rfc3339");
	const parts = {
		year: groupNumber(groups, "year"),
		month: groupNumber(groups, "month"),
		day: groupNumber(groups, "day"),
		hour: groupNumber(groups, "hour"),
		minute: groupNumber(groups, "minute"),
		second: groupNumber(groups, "second"),
		millisecond: fractionalMilliseconds(groups["fraction"])
	};
	if (parts.year === 0 || parts.hour > 23 || parts.minute > 59 || parts.second > 59) return rejected("out_of_range");
	const local = plainDateTime(parts);
	if (local === void 0) return rejected("not_a_calendar_date");
	const localEpoch = local.toZonedDateTime("UTC").epochMilliseconds;
	if (groups["zone"] === "Z") return Object.freeze({
		kind: "accepted",
		epoch: localEpoch
	});
	const offsetHour = groupNumber(groups, "offsetHour");
	const offsetMinute = groupNumber(groups, "offsetMinute");
	if (offsetHour > 23 || offsetMinute > 59 || groups["sign"] === "-" && offsetHour === 0 && offsetMinute === 0) return rejected("invalid_numeric_offset");
	const direction = groups["sign"] === "+" ? 1 : -1;
	return Object.freeze({
		kind: "accepted",
		epoch: localEpoch - direction * (offsetHour * 60 + offsetMinute) * 6e4
	});
}
/**
* Validate one canonical four-digit-year RFC 3339 UTC instant.
*
* The accepted profile carries exactly three fractional digits and a `Z`, and
* every year from 0001 through 9999 whose calendar date exists is representable.
* @param value - Untrusted candidate instant.
* @returns The epoch milliseconds, or the reason the text is not the canonical profile.
*/
function fourDigitYearEpoch(value) {
	if (!UTC_INSTANT.test(value)) return rejected("not_rfc3339");
	const epoch = Date.parse(value);
	if (!Number.isFinite(epoch) || !inFourDigitYearRange(epoch) || new Date(epoch).toISOString() !== value) return rejected("not_a_calendar_date");
	return Object.freeze({
		kind: "accepted",
		epoch
	});
}
/**
* Whether an epoch is representable as a canonical four-digit-year RFC 3339 UTC instant.
*
* Callers that only need the bound use this instead of re-deriving it from the
* two constants, so the range has one owner.
* @param epoch - Candidate epoch milliseconds.
* @returns Whether the epoch is a safe integer inside the four-digit-year range.
*/
function isFourDigitYearInstant(epoch) {
	return inFourDigitYearRange(epoch);
}
/**
* Format one epoch as the canonical four-digit-year RFC 3339 UTC instant.
*
* The bound check runs first, so a caller that also compares the result with a
* decision time never formats an unrepresentable instant.
* @param epoch - Candidate epoch milliseconds; a non-integral value is unrepresentable.
* @returns The canonical instant text, or `undefined` when the epoch is unrepresentable.
*/
function formatFourDigitYearInstant(epoch) {
	if (!inFourDigitYearRange(epoch)) return void 0;
	const instant = new Date(epoch).toISOString();
	/* v8 ignore next -- an in-range integral Date always formats as the canonical UTC profile. */
	if (!UTC_INSTANT.test(instant)) return void 0;
	return instant;
}
/**
* Validate and canonicalize one raw IANA zone selector.
*
* A zone identity is stored on durable records and resolved again by another
* process, so an alias is answered by its platform-canonical name. Abbreviations,
* numeric offsets, single-segment names, and unknown names are refused.
* @param value - Candidate `UTC` or IANA Area/Location name.
* @returns The canonical zone, or the reason the name is unusable.
*/
function canonicalizeTimeZone(value) {
	if (value.length === 0) return rejected("blank");
	if (value.trim() !== value) return rejected("padded");
	if (value !== "UTC" && !IANA_TIME_ZONE.test(value)) return rejected("not_area_location");
	let canonical;
	try {
		canonical = new Intl.DateTimeFormat("en-US", { timeZone: value }).resolvedOptions().timeZone;
	} catch (error) {
		return Object.freeze({
			kind: "rejected",
			reason: "unsupported",
			cause: error
		});
	}
	/* v8 ignore next -- Intl answers UTC or a canonical IANA Area/Location for every accepted name. */
	if (canonical !== "UTC" && !IANA_TIME_ZONE.test(canonical)) return rejected("not_canonical_shape");
	return Object.freeze({
		kind: "accepted",
		timeZone: canonical
	});
}
/**
* Resolve one local wall-clock value in an explicit zone.
*
* A value inside a forward transition's gap does not exist, so it answers
* `undefined`; a value inside an overlap resolves to the earlier instant, which
* is the same choice the maintained `Temporal` implementation makes.
* @param time - Exact calendar fields of a real date, or an already-built plain date-time.
* @param timeZone - Already-canonical `UTC` or IANA zone.
* @returns The resolved epoch milliseconds, or `undefined` for a nonexistent wall-clock time.
* @throws RangeError when `timeZone` is not a known zone or the calendar value does not exist.
*/
function resolveLocalInstant(time, timeZone) {
	const local = time instanceof Temporal.PlainDateTime ? time : Temporal.PlainDateTime.from(time, { overflow: "reject" });
	const zoned = local.toZonedDateTime(timeZone, { disambiguation: "earlier" });
	return zoned.toPlainDateTime().equals(local) ? zoned.epochMilliseconds : void 0;
}
//#endregion
//#region lib/types/duration.js
/**
* One span of time, held as a count of milliseconds and split into the whole
* parts a reading is written in.
*
* A class rather than a branded number: arithmetic on two branded numbers
* answers an unbranded `number`, so a brand cannot stop seconds and
* milliseconds from being added, while an instance can. The published surface
* is the set the migrated consumers call; a further member lands with the caller
* that needs it.
* @module @deepseek-ai/dsh-util-time
*/
const MS_PER_SECOND = 1e3;
const SECONDS_PER_DAY = 86400;
const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_MINUTE = 60;
/**
* Largest millisecond count a span holds.
*
* Every integer up to this bound is exactly representable, which is what makes
* the whole-part split exact; beyond it the parts a reader gets back would not
* add up to the span.
*/
const MAX_SPAN_MS = Number.MAX_SAFE_INTEGER;
/**
* One span of time in milliseconds.
*
* Instances are frozen, so a span cannot drift after it is built. A span is
* signed: a negative count builds, because the clock difference that produced it
* is real, but `decompose()` refuses it because a negative interval has no single
* whole-part split. A caller that reports parts therefore decides its own policy
* first, and the elapsed reading clamps a backward clock reading to zero before
* it builds the span. A span is bounded by the exactly representable millisecond
* range, which is wider than any interval this harness schedules, and it carries
* no calendar meaning: a month is not a fixed count of milliseconds.
*/
var Duration = class Duration {
	span;
	constructor(milliseconds) {
		this.span = milliseconds;
		Object.freeze(this);
	}
	/**
	* Build one span from a millisecond count.
	* A negative count is accepted, so a caller can hold a backward clock difference;
	* such a span cannot be decomposed, and the caller states its own sign policy.
	* @param milliseconds - Signed count within the exactly representable range (about 285,000 years); fractions are kept.
	* @returns The span.
	* @throws TypeError when the count is not finite, or exceeds the exactly representable millisecond range.
	*/
	static fromMilliseconds(milliseconds) {
		if (!Number.isFinite(milliseconds)) throw new TypeError(`duration milliseconds must be finite, got ${String(milliseconds)}`);
		if (Math.abs(milliseconds) > MAX_SPAN_MS) throw new TypeError(`duration milliseconds must not exceed ${MAX_SPAN_MS} so its parts stay exact, got ${String(milliseconds)}`);
		return new Duration(milliseconds);
	}
	/**
	* Split the span into whole days, hours, minutes, seconds, and the remaining
	* fraction of a second. Days, hours, minutes, and seconds are floored; the
	* millisecond part keeps the fraction below one second. Every span this class
	* accepts is inside the exactly representable millisecond range, so the parts
	* add back up to the span.
	* @returns The frozen whole parts.
	* @throws RangeError when the span is negative, because a negative interval
	* has no single whole-part decomposition: the caller owns that policy.
	*/
	decompose() {
		if (this.span < 0) throw new RangeError("a negative duration has no whole-part decomposition");
		let seconds = Math.floor(this.span / MS_PER_SECOND);
		const days = Math.floor(seconds / SECONDS_PER_DAY);
		seconds %= SECONDS_PER_DAY;
		const hours = Math.floor(seconds / SECONDS_PER_HOUR);
		seconds %= SECONDS_PER_HOUR;
		const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
		seconds %= SECONDS_PER_MINUTE;
		return Object.freeze({
			days,
			hours,
			minutes,
			seconds,
			milliseconds: this.span - Math.floor(this.span / MS_PER_SECOND) * MS_PER_SECOND
		});
	}
};
//#endregion
//#region lib/types/recurrence.js
/** Bounded calendar recurrence shared by consumers of Cron and RRULE. */
/** Internal validation failure, contained by the public parsers. */
var RecurrenceInputError = class extends Error {
	reason;
	constructor(reason, message, options) {
		super(message, options);
		this.reason = reason;
	}
};
/**
* Contain recurrence validation failures while preserving unexpected implementation errors.
* @param parse - Synchronous rule constructor whose input failures use RecurrenceInputError.
* @returns The parsed value or its validation refusal.
*/
function parseRecurrence(parse) {
	try {
		return {
			kind: "accepted",
			rule: parse()
		};
	} catch (error) {
		if (!(error instanceof RecurrenceInputError)) throw error;
		return {
			kind: "rejected",
			reason: error.reason,
			detail: error.message,
			...error.cause === void 0 ? {} : { cause: error.cause }
		};
	}
}
/**
* Validate the explicit decision and its committed lower bound before calendar arithmetic.
* @param scheduledAt - Committed canonical four-digit-year UTC occurrence with milliseconds.
* @param acceptedAt - Explicit epoch milliseconds of the delivery decision.
* @returns The committed epoch, or throws RangeError for an invalid decision.
*/
function recurrenceDecision(scheduledAt, acceptedAt) {
	const savedTarget = fourDigitYearEpoch(scheduledAt);
	if (!isFourDigitYearInstant(acceptedAt) || savedTarget.kind === "rejected" || acceptedAt < savedTarget.epoch) throw new RangeError("Recurrence decision must be representable and at or after its committed target");
	return savedTarget.epoch;
}
//#endregion
//#region lib/types/cron.js
/** Strict five-field Cron parsing and bounded, zone-aware occurrence arithmetic. */
/** Gregorian calendar repetition bounds both forward and backward searches. */
const CRON_SEARCH_HORIZON_YEARS = 400;
/** Project an explicit instant into a calendar date in the rule's zone. */
function localDate(epoch, timeZone) {
	return Temporal.Instant.fromEpochMilliseconds(epoch).toZonedDateTimeISO(timeZone).toPlainDate();
}
const CRON_FIELDS = [
	{
		name: "minute",
		min: 0,
		max: 59,
		canonicalMax: 59
	},
	{
		name: "hour",
		min: 0,
		max: 23,
		canonicalMax: 23
	},
	{
		name: "day-of-month",
		min: 1,
		max: 31,
		canonicalMax: 31
	},
	{
		name: "month",
		min: 1,
		max: 12,
		canonicalMax: 12
	},
	{
		name: "day-of-week",
		min: 0,
		max: 7,
		canonicalMax: 6
	}
];
/** Build the stable diagnostic for one malformed cron field element. */
function invalidCronField(spec, element) {
	return new RecurrenceInputError("syntax", `cron.expression ${spec.name} field element ${JSON.stringify(element)} must be *, a value, a-b, */n, a-b/n, or a comma-separated list of those.`);
}
/** Read one in-range cron field value. */
function cronFieldValue(text, spec) {
	const value = Number(text);
	if (!Number.isSafeInteger(value) || value < spec.min || value > spec.max) throw new RecurrenceInputError("syntax", `cron.expression ${spec.name} field value ${text} is outside ${spec.min}-${spec.max}.`);
	return value;
}
/** Read one positive cron field step. */
function cronFieldStep(text, spec) {
	const step = Number(text);
	if (!Number.isSafeInteger(step) || step < 1) throw new RecurrenceInputError("syntax", `cron.expression ${spec.name} field step must be a positive integer.`);
	return step;
}
/** Expand one comma-separated cron field into its matched value set. */
function cronFieldValues(raw, spec) {
	const matched = /* @__PURE__ */ new Set();
	for (const element of raw.split(",")) {
		if (element.length === 0) throw new RecurrenceInputError("syntax", `cron.expression ${spec.name} field must not contain an empty list element.`);
		if (element === "*") {
			for (let value = spec.min; value <= spec.max; value += 1) matched.add(value);
			continue;
		}
		if (element.startsWith("*")) {
			const groups = /^\*\/(?<step>\d+)$/.exec(element)?.groups;
			if (groups === void 0) throw invalidCronField(spec, element);
			const stepText = groups["step"];
			/* v8 ignore next -- a successful fixed regex always provides the step group. */
			if (stepText === void 0) throw invalidCronField(spec, element);
			const step = cronFieldStep(stepText, spec);
			for (let value = spec.min; value <= spec.max; value += step) matched.add(value);
			continue;
		}
		const groups = /^(?<start>\d+)(?:-(?<end>\d+))?(?:\/(?<step>\d+))?$/.exec(element)?.groups;
		if (groups === void 0) throw invalidCronField(spec, element);
		const startText = groups["start"];
		/* v8 ignore next -- a successful fixed regex always provides the start group. */
		if (startText === void 0) throw invalidCronField(spec, element);
		const start = cronFieldValue(startText, spec);
		const end = groups["end"];
		const stepText = groups["step"];
		if (end === void 0) {
			if (stepText !== void 0) throw invalidCronField(spec, element);
			matched.add(start);
			continue;
		}
		const last = cronFieldValue(end, spec);
		if (start > last) throw new RecurrenceInputError("syntax", `cron.expression ${spec.name} field range ${start}-${last} is inverted.`);
		const step = stepText === void 0 ? 1 : cronFieldStep(stepText, spec);
		for (let value = start; value <= last; value += step) matched.add(value);
	}
	const values = /* @__PURE__ */ new Set();
	for (const value of matched) values.add(spec.canonicalMax !== spec.max && value === spec.max ? spec.min : value);
	return [...values].sort((left, right) => left - right);
}
/**
* Read one value the encoder already proved present.
* @param values - matched value set the parser always fills.
* @param index - index the caller derived from that set.
* @returns the value at that index.
*/
function cronFieldValueAt(values, index) {
	const value = values[index];
	/* v8 ignore next -- a parsed cron field always matches at least one value. */
	if (value === void 0) throw new Error("cron field encoding requires a non-empty value set");
	return value;
}
/** Encode one matched value set as the shortest equivalent comma-separated cron field.
* @param values - Matched values in ascending order.
* @returns Field text that re-parses to exactly `values`, never spelled with a leading `*`.
*/
function encodeCronField(values) {
	const first = cronFieldValueAt(values, 0);
	if (values.length === 1) return String(first);
	const last = cronFieldValueAt(values, values.length - 1);
	const step = cronFieldValueAt(values, 1) - first;
	let uniform = true;
	for (let index = 2; index < values.length; index += 1) if (cronFieldValueAt(values, index) - cronFieldValueAt(values, index - 1) !== step) {
		uniform = false;
		break;
	}
	if (uniform) {
		if (step === 1) return `${first}-${last}`;
		return `${first}-${last}/${step}`;
	}
	const parts = [];
	let runStart = first;
	for (let index = 1; index < values.length; index += 1) {
		const current = cronFieldValueAt(values, index);
		const previous = cronFieldValueAt(values, index - 1);
		if (current === previous + 1) continue;
		parts.push(runStart === previous ? String(runStart) : `${runStart}-${previous}`);
		runStart = current;
	}
	const final = cronFieldValueAt(values, values.length - 1);
	parts.push(runStart === final ? String(runStart) : `${runStart}-${final}`);
	return parts.join(",");
}
/**
* One uniform walk from a field's minimum, folded exactly as a parsed field folds.
* @param spec - Field range and canonical maximum.
* @param step - Positive step of the walk.
* @returns Ascending values `step` produces from the minimum.
*/
function starWalk(spec, step) {
	const walked = /* @__PURE__ */ new Set();
	for (let value = spec.min; value <= spec.max; value += step) walked.add(spec.canonicalMax !== spec.max && value === spec.max ? spec.min : value);
	return [...walked].sort((left, right) => left - right);
}
/**
* Canonical spelling of a field whose text started with `*`.
*
* Only a `*`-prefixed spelling keeps the star flag, and only the bare star or a star-step
* (a `*` followed by `/n`) admits a leading `*`, so the encoding is the bare star for every
* value, otherwise the widest star-step walk plus any remaining values as a list.
* Re-parsing therefore reproduces both the star flag and the matched set, which is what
* keeps day-of-month/day-of-week AND/OR selection stable across a stored canonical
* expression.
* @param values - Matched values in ascending order.
* @param spec - Field range and canonical maximum.
* @returns Canonical field text that starts with `*`.
*/
function encodeStarCronField(values, spec) {
	if (values.length === spec.canonicalMax - spec.min + 1) return "*";
	const present = new Set(values);
	let bestStep;
	let bestWalk = [];
	for (let step = 1; step <= spec.max - spec.min + 1; step += 1) {
		const walk = starWalk(spec, step);
		if (walk.length > bestWalk.length && walk.every((value) => present.has(value))) {
			bestStep = step;
			bestWalk = walk;
		}
	}
	/* v8 ignore next -- the widest step yields the single minimum, which every `*`-led field matches. */
	if (bestStep === void 0) throw new RecurrenceInputError("syntax", `${spec.name} cannot keep a leading \`*\`.`);
	const walked = new Set(bestWalk);
	const remaining = values.filter((value) => !walked.has(value));
	const walkText = `*/${bestStep}`;
	return remaining.length === 0 ? walkText : `${walkText},${encodeCronField(remaining)}`;
}
/**
* Parse one cron field into its matched values, canonical spelling, and star flag.
*
* The star flag tests the field text's first character, which is how Vixie sets
* `DOM_STAR`/`DOW_STAR`: a stepped star (`*` followed by `/2`) is a star although
* it matches half the range, while an explicit full range such as `1-31` is
* restricted. Canonicalization preserves the flag: a `*`-led field keeps a `*`-led
* spelling, and a field that did not start with `*` is never spelled as a star-step.
*/
function parseCronField(raw, spec) {
	const values = cronFieldValues(raw, spec);
	const star = raw.startsWith("*");
	return Object.freeze({
		values: Object.freeze(values),
		canonical: star ? encodeStarCronField(values, spec) : encodeCronField(values),
		star
	});
}
/** Parse and canonicalize one strict five-field cron expression. */
function parseCronExpression(expression) {
	if (typeof expression !== "string" || expression.length === 0 || expression.trim() !== expression) throw new RecurrenceInputError("syntax", "cron.expression must be a non-empty trimmed string.");
	const fields = expression.split(/\s+/);
	if (fields.length !== 5) throw new RecurrenceInputError("syntax", "cron.expression must contain exactly five whitespace-separated fields: minute hour day-of-month month day-of-week.");
	const [minute, hour, dayOfMonth, month, dayOfWeek] = fields.map((field, index) => {
		const fieldSpec = CRON_FIELDS[index];
		/* v8 ignore next -- the five-field split bounds this index. */
		if (fieldSpec === void 0) throw new Error("cron field specification is missing");
		return parseCronField(field, fieldSpec);
	});
	return Object.freeze({
		expression: [
			minute,
			hour,
			dayOfMonth,
			month,
			dayOfWeek
		].map((field) => field.canonical).join(" "),
		minutes: minute.values,
		hours: hour.values,
		daysOfMonth: dayOfMonth.values,
		months: month.values,
		daysOfWeek: dayOfWeek.values,
		dayOfMonthStar: dayOfMonth.star,
		dayOfWeekStar: dayOfWeek.star
	});
}
/**
* Build the local-date predicate of one parsed cron rule under Vixie day-of-month/day-of-week semantics.
*
* The star flag selects the branch only; it never excuses a field's matched values.
* When either field text starts with `*`, a local date must satisfy BOTH fields, so a
* stepped star such as `*` followed by `/2` still restricts the dates it matches. A bare
* `*` matches every value, which makes that field's match always true and degrades the
* AND to the other field, exactly like Vixie's `DOM_STAR`/`DOW_STAR` test. Only when
* neither field text starts with `*` does either field matching suffice.
* @param parsed - Parsed cron rule with canonical field text and matched values.
* @returns Whether one local date matches the rule.
*/
function cronDateMatch(parsed) {
	const months = new Set(parsed.months);
	const daysOfMonth = new Set(parsed.daysOfMonth);
	const daysOfWeek = new Set(parsed.daysOfWeek);
	const dayOfMonthRestricted = !parsed.dayOfMonthStar;
	const dayOfWeekRestricted = !parsed.dayOfWeekStar;
	return (date) => {
		/* v8 ignore next -- both callers pre-filter by month before matching a date. */
		if (!months.has(date.month)) return false;
		const dayOfMonth = daysOfMonth.has(date.day);
		const dayOfWeek = daysOfWeek.has(date.dayOfWeek % 7);
		if (dayOfMonthRestricted && dayOfWeekRestricted) return dayOfMonth || dayOfWeek;
		return dayOfMonth && dayOfWeek;
	};
}
/** Enumerate local minutes of day without constructing unvisited calendar values. */
function* cronTimes(parsed, reverse = false) {
	const hours = reverse ? parsed.hours.toReversed() : parsed.hours;
	const minutes = reverse ? parsed.minutes.toReversed() : parsed.minutes;
	for (const hour of hours) for (const minute of minutes) yield hour * 60 + minute;
}
/**
* Find the first strictly future cron occurrence without leaving the four-digit UTC year range.
*
* The walk stops at {@link CRON_SEARCH_HORIZON_YEARS} past the floor date when that
* horizon precedes the four-digit ceiling: a rule with no match inside the horizon
* never matches, so its search resolves to exhaustion instead of scanning to the ceiling.
* @param parsed - Parsed cron rule.
* @param timeZone - Explicit zone interpreting its local date and time.
* @param floor - Instant every returned target must exceed.
* @returns The first matching target, or exhaustion as undefined.
*/
function nextCronTarget(parsed, timeZone, floor) {
	const matches = cronDateMatch(parsed);
	const months = new Set(parsed.months);
	const localFloor = Temporal.Instant.fromEpochMilliseconds(floor).toZonedDateTimeISO(timeZone);
	const firstDate = localFloor.toPlainDate();
	let date = firstDate;
	const floorMinute = localFloor.hour * 60 + localFloor.minute;
	const ceiling = localDate(MAX_FOUR_DIGIT_YEAR_MS, "UTC").add({ days: 1 });
	const horizon = date.add({ years: 400 });
	const lastDate = Temporal.PlainDate.compare(horizon, ceiling) < 0 ? horizon : ceiling;
	while (Temporal.PlainDate.compare(date, lastDate) <= 0) {
		if (!months.has(date.month)) {
			date = date.add({ months: 1 }).with({ day: 1 });
			continue;
		}
		if (matches(date)) for (const minute of cronTimes(parsed)) {
			if (date.equals(firstDate) && minute < floorMinute) continue;
			const target = resolveLocalInstant(date.toPlainDateTime({
				hour: Math.floor(minute / 60),
				minute: minute % 60
			}), timeZone);
			if (target === void 0) continue;
			if (target > MAX_FOUR_DIGIT_YEAR_MS) return void 0;
			if (target >= MIN_FOUR_DIGIT_YEAR_MS && target > floor) return new Date(target).toISOString();
		}
		date = date.add({ days: 1 });
	}
}
/**
* Find the latest matching cron occurrence at or before a decision without leaving the committed floor.
*
* The walk stops at {@link CRON_SEARCH_HORIZON_YEARS} before the decision date when that
* horizon follows the four-digit floor, which is the local date holding that floor instant
* in the rule's own zone: a rule with no match inside the horizon keeps the committed target
* instead of scanning back to the floor, and a candidate before the floor instant is skipped.
* @param parsed - Parsed cron rule.
* @param timeZone - Explicit zone interpreting its local date and time.
* @param decision - Wall-clock decision time in epoch milliseconds.
* @param savedTarget - Committed target that stays due even when current zone data resolves past it.
* @returns The latest due occurrence, or the committed target when no local occurrence is representable.
*/
function latestCronOccurrence(parsed, timeZone, decision, savedTarget) {
	const matches = cronDateMatch(parsed);
	const months = new Set(parsed.months);
	let date = localDate(decision, "UTC").add({ days: 1 });
	const floorDate = localDate(MIN_FOUR_DIGIT_YEAR_MS, timeZone);
	const horizon = date.subtract({ years: 400 });
	const firstDate = Temporal.PlainDate.compare(horizon, floorDate) > 0 ? horizon : floorDate;
	while (Temporal.PlainDate.compare(date, firstDate) >= 0) {
		if (!months.has(date.month)) {
			date = date.with({ day: 1 }).subtract({ days: 1 });
			continue;
		}
		if (matches(date)) {
			let earliest;
			for (const minute of cronTimes(parsed)) {
				earliest = resolveLocalInstant(date.toPlainDateTime({
					hour: Math.floor(minute / 60),
					minute: minute % 60
				}), timeZone);
				if (earliest !== void 0) break;
			}
			if (earliest !== void 0 && earliest > decision) {
				date = date.subtract({ days: 1 });
				continue;
			}
			for (const minute of cronTimes(parsed, true)) {
				const candidate = resolveLocalInstant(date.toPlainDateTime({
					hour: Math.floor(minute / 60),
					minute: minute % 60
				}), timeZone);
				if (candidate === void 0 || candidate > decision || candidate < MIN_FOUR_DIGIT_YEAR_MS) continue;
				return Math.max(candidate, savedTarget);
			}
		}
		date = date.subtract({ days: 1 });
	}
	return savedTarget;
}
/** Parsed Cron value; expression and zone stay authoritative independently of any task. */
var Cron = class Cron {
	parsed;
	timeZone;
	constructor(parsed, timeZone) {
		this.parsed = parsed;
		this.timeZone = timeZone;
	}
	/** Immutable matched fields and canonical text for consumers such as rule descriptions. */
	get fields() {
		return this.parsed;
	}
	/** Canonical five-field expression, preserving Vixie star semantics. */
	get expression() {
		return this.parsed.expression;
	}
	/**
	* Parse one five-field expression and its explicit IANA zone.
	* @param expression - Five numeric Cron fields with Vixie day matching.
	* @param timeZone - UTC or an IANA Area/Location zone.
	* @returns A parsed value, or a syntax/zone refusal without task policy.
	*/
	static parse(expression, timeZone) {
		return parseRecurrence(() => {
			const parsed = parseCronExpression(expression);
			const zone = canonicalizeTimeZone(timeZone);
			if (zone.kind === "rejected") throw new RecurrenceInputError("time_zone", "Cron requires a valid UTC or IANA Area/Location zone.", { cause: zone });
			const rule = new Cron(parsed, zone.timeZone);
			Object.freeze(rule);
			return rule;
		});
	}
	/**
	* First strictly future occurrence, skipping gaps and choosing the earlier overlap.
	* @param after - Representable epoch milliseconds that the occurrence must exceed.
	* @returns The UTC occurrence, or exhaustion of the bounded search.
	*/
	next(after) {
		if (!isFourDigitYearInstant(after)) throw new RangeError("Cron lower bound must be a four-digit-year instant");
		return nextCronTarget(this.parsed, this.timeZone, after);
	}
	/**
	* Latest due occurrence and first future occurrence, without replaying a backlog.
	* @param scheduledAt - Canonical four-digit-year UTC occurrence with milliseconds, retained across zone-data changes.
	* @param acceptedAt - Representable decision time at or after that occurrence.
	* @returns The latest due occurrence and optional future successor.
	* @throws RangeError when either instant is invalid or the decision precedes the committed occurrence.
	*/
	resolve(scheduledAt, acceptedAt) {
		const savedTarget = recurrenceDecision(scheduledAt, acceptedAt);
		const occurrenceAt = new Date(latestCronOccurrence(this.parsed, this.timeZone, acceptedAt, savedTarget)).toISOString();
		const nextScheduledAt = this.next(acceptedAt);
		return Object.freeze(nextScheduledAt === void 0 ? { occurrenceAt } : {
			occurrenceAt,
			nextScheduledAt
		});
	}
};
//#endregion
//#region lib/types/rrule.js
/** Bounded RFC 5545 recurrence over UTC or explicit local wall time. */
/** Require a canonical zone while retaining the parser's validation result. */
function validatedZone(value) {
	const result = canonicalizeTimeZone(value);
	if (result.kind === "rejected") throw new RecurrenceInputError("time_zone", "TZID must be UTC or a valid IANA Area/Location name.", { cause: result });
	return result.timeZone;
}
const { Weekday, rrulestr } = rrulePackage;
/** Largest accepted COUNT; a longer finite recurrence uses UNTIL. */
const MAX_RRULE_COUNT = 1e3;
/** Largest number of occurrences that one FREQ period may generate: every minute of one week. */
const MAX_PERIOD_CANDIDATES = 10080;
/** Largest number of BYSETPOS values. */
const MAX_SET_POSITIONS = 31;
/** Earliest DTSTART and UNTIL year; the recurrence library reads years 0000 through 0099 as 1900 through 1999. */
const MIN_RRULE_YEAR = 100;
const DAY_MS = 864e5;
/** Latest wall time that the recurrence library can generate or read as a DTSTART. */
const LAST_RRULE_WALL_MS = Date.UTC(9999, 11, 31, 23, 59, 59);
/** FREQ periods in one 400-year Gregorian cycle; after a cycle, weekdays and leap days repeat. */
const CYCLE_PERIODS = {
	MINUTELY: 210379680,
	HOURLY: 3506328,
	DAILY: 146097,
	WEEKLY: 20871,
	MONTHLY: 4800,
	YEARLY: 400
};
/** Length of one 400-year Gregorian cycle in milliseconds. */
const CYCLE_MS = CYCLE_PERIODS.DAILY * DAY_MS;
/** RFC 5545 weekday codes; index 0 is Monday, as in the recurrence library. */
const WEEKDAYS = [
	"MO",
	"TU",
	"WE",
	"TH",
	"FR",
	"SA",
	"SU"
];
const SUPPORTED_PARTS = [
	"FREQ",
	"INTERVAL",
	"COUNT",
	"UNTIL",
	"WKST",
	"BYSECOND",
	"BYMINUTE",
	"BYHOUR",
	"BYDAY",
	"BYMONTHDAY",
	"BYYEARDAY",
	"BYWEEKNO",
	"BYMONTH",
	"BYSETPOS"
];
const LIST_RANGES = new Map([
	["BYSECOND", {
		min: 0,
		max: 59
	}],
	["BYMINUTE", {
		min: 0,
		max: 59
	}],
	["BYHOUR", {
		min: 0,
		max: 23
	}],
	["BYMONTHDAY", {
		min: -31,
		max: 31
	}],
	["BYYEARDAY", {
		min: -366,
		max: 366
	}],
	["BYWEEKNO", {
		min: -53,
		max: 53
	}],
	["BYMONTH", {
		min: 1,
		max: 12
	}],
	["BYSETPOS", {
		min: -366,
		max: 366
	}]
]);
/** BY lists whose values the recurrence library combines into the times of one day. */
const TIME_PARTS = [
	"BYHOUR",
	"BYMINUTE",
	"BYSECOND"
];
const DTSTART_UTC = /^DTSTART:\d{8}T\d{6}Z$/;
const DTSTART_ZONED = /^DTSTART;TZID=[^:;]+:\d{8}T\d{6}$/;
const BASIC_UTC = /^\d{8}T\d{6}Z$/;
const UNSIGNED_INTEGER = /^\d+$/;
const SIGNED_INTEGER = /^[+-]?\d+$/;
const WEEKDAY_SELECTOR = /^(?:[+-]?(?:[1-9]|[1-4]\d|5[0-3]))?(?:MO|TU|WE|TH|FR|SA|SU)$/;
/** Read a matched basic date-time `YYYYMMDDTHHMMSS` as UTC-shaped epoch milliseconds, rejecting normalization. */
function basicTime(field, text) {
	const year = Number(text.slice(0, 4));
	const month = Number(text.slice(4, 6));
	const day = Number(text.slice(6, 8));
	const hour = Number(text.slice(9, 11));
	const minute = Number(text.slice(11, 13));
	const second = Number(text.slice(13, 15));
	const message = `${field} must be a real calendar date and time in years 0100 through 9999.`;
	if (year < MIN_RRULE_YEAR) throw new RecurrenceInputError("syntax", message);
	try {
		Temporal.PlainDateTime.from({
			year,
			month,
			day,
			hour,
			minute,
			second
		}, { overflow: "reject" });
	} catch (error) {
		throw new RecurrenceInputError("syntax", message, { cause: error });
	}
	return Date.UTC(year, month - 1, day, hour, minute, second);
}
/** Split the input into its trimmed DTSTART and RRULE lines. */
function recurrenceLines(value) {
	const [startLine, ruleLine, ...rest] = value.replaceAll("\r\n", "\n").trim().split("\n").map((line) => line.trim()).filter(Boolean);
	if (startLine === void 0 || ruleLine === void 0 || rest.length > 0 || !startLine.startsWith("DTSTART") || !ruleLine.startsWith("RRULE:")) throw new RecurrenceInputError("syntax", "rrule must contain one DTSTART line followed by one RRULE line.");
	return [startLine, ruleLine];
}
/** Read DTSTART as its canonical zone and UTC-shaped wall time. */
function parseStart(line) {
	if (DTSTART_UTC.test(line)) return {
		timeZone: "UTC",
		start: basicTime("DTSTART", line.slice(-16, -1))
	};
	if (!DTSTART_ZONED.test(line)) throw new RecurrenceInputError("syntax", "DTSTART must use UTC with Z or a local date-time with an explicit TZID.");
	return {
		timeZone: validatedZone(line.slice(13, -16)),
		start: basicTime("DTSTART", line.slice(-15))
	};
}
/** Split the RRULE line into its parts in input order. */
function ruleParts(line) {
	const parts = /* @__PURE__ */ new Map();
	for (const component of line.slice(6).split(";")) {
		const separator = component.indexOf("=");
		const key = component.slice(0, separator);
		const text = component.slice(separator + 1);
		if (separator <= 0 || text.length === 0 || parts.has(key)) throw new RecurrenceInputError("syntax", "RRULE fields must be non-empty and unique.");
		if (!SUPPORTED_PARTS.includes(key)) throw new RecurrenceInputError("syntax", `RRULE supports only ${SUPPORTED_PARTS.join(", ")}.`);
		parts.set(key, text);
	}
	return parts;
}
/** Recognize a FREQ value that this module enumerates. */
function isFrequency(value) {
	return Object.hasOwn(CYCLE_PERIODS, value);
}
/** Read FREQ; SECONDLY is valid RFC 5545 but violates the 60-second occurrence spacing. */
function frequencyOf(text) {
	if (text === void 0) throw new RecurrenceInputError("syntax", "RRULE must contain FREQ.");
	if (text === "SECONDLY") throw new RecurrenceInputError("density", "FREQ=SECONDLY is not supported; occurrences must be at least 60 seconds apart.");
	if (!isFrequency(text)) throw new RecurrenceInputError("syntax", "FREQ must be MINUTELY, HOURLY, DAILY, WEEKLY, MONTHLY, or YEARLY.");
	return text;
}
/** Read an optional unsigned part from 1 through max. */
function positiveValue(text, max, message) {
	if (text === void 0) return void 0;
	const value = Number(text);
	if (!UNSIGNED_INTEGER.test(text) || value < 1 || value > max) throw new RecurrenceInputError("syntax", message);
	return value;
}
/** Read an optional UNTIL as an actual UTC instant. */
function untilOf(text) {
	if (text === void 0) return void 0;
	if (!BASIC_UTC.test(text)) throw new RecurrenceInputError("syntax", "UNTIL must be an absolute UTC date-time ending in Z.");
	return basicTime("UNTIL", text.slice(0, -1));
}
/** Read WKST as a weekday index; RFC 5545 defaults it to Monday. */
function weekStartOf(text) {
	const index = WEEKDAYS.indexOf(text ?? "MO");
	if (index < 0) throw new RecurrenceInputError("syntax", "WKST must be MO, TU, WE, TH, FR, SA, or SU.");
	return index;
}
/** Read one numeric BY list whose values are unique and in range. */
function listValues(key, text, range) {
	const signed = range.min < 0;
	const values = text.split(",").map((item) => {
		const value = Number(item);
		if (!(signed ? SIGNED_INTEGER : UNSIGNED_INTEGER).test(item) || value < range.min || value > range.max || signed && value === 0) throw new RecurrenceInputError("syntax", `${key} values must be integers from ${range.min} to ${range.max}${signed ? ", excluding 0" : ""}.`);
		return value;
	});
	if (new Set(values).size !== values.length) throw new RecurrenceInputError("syntax", `${key} values must be unique.`);
	return values;
}
/** Read an optional BYDAY list; `+2TU` and `2TU` select the same day. */
function byDayOf(text) {
	if (text === void 0) return void 0;
	const selectors = text.split(",");
	const seen = /* @__PURE__ */ new Set();
	let ordinals = 0;
	for (const selector of selectors) {
		if (!WEEKDAY_SELECTOR.test(selector)) throw new RecurrenceInputError("syntax", "BYDAY contains an invalid weekday selector.");
		const ordinal = selector.slice(0, -2);
		seen.add(`${Number(ordinal)}${selector.slice(-2)}`);
		if (ordinal.length > 0) ordinals += 1;
	}
	if (seen.size !== selectors.length) throw new RecurrenceInputError("syntax", "BYDAY values must be unique.");
	return {
		entries: selectors.length,
		ordinals
	};
}
/** Decode every RRULE part value. */
function ruleValues(parts) {
	const frequency = frequencyOf(parts.get("FREQ"));
	const interval = positiveValue(parts.get("INTERVAL"), Number.MAX_SAFE_INTEGER, "INTERVAL must be a positive safe integer.");
	const count = positiveValue(parts.get("COUNT"), MAX_RRULE_COUNT, `COUNT must be an integer from 1 to ${MAX_RRULE_COUNT}; use UNTIL for a longer finite recurrence.`);
	const until = untilOf(parts.get("UNTIL"));
	const weekStart = weekStartOf(parts.get("WKST"));
	const lists = /* @__PURE__ */ new Map();
	for (const [key, range] of LIST_RANGES) {
		const text = parts.get(key);
		if (text !== void 0) lists.set(key, listValues(key, text, range));
	}
	return {
		frequency,
		interval: interval ?? 1,
		count,
		until,
		weekStart,
		lists,
		byDay: byDayOf(parts.get("BYDAY"))
	};
}
/** Reject a part combination that RFC 5545 forbids or that breaks the 60-second occurrence spacing. */
function checkCombination(values) {
	const { frequency, lists, byDay } = values;
	const ordinals = byDay?.ordinals ?? 0;
	if (values.count !== void 0 && values.until !== void 0) throw new RecurrenceInputError("syntax", "RRULE must not contain both COUNT and UNTIL.");
	if ((lists.get("BYSECOND")?.length ?? 1) > 1) throw new RecurrenceInputError("density", "BYSECOND must contain one value; occurrences must be at least 60 seconds apart.");
	if ((frequency === "MINUTELY" || frequency === "HOURLY") && (lists.size > 0 || byDay !== void 0)) throw new RecurrenceInputError("syntax", `FREQ=${frequency} does not support BY rule parts; use FREQ=DAILY or FREQ=WEEKLY with BYHOUR and BYMINUTE.`);
	for (const key of ["BYWEEKNO", "BYYEARDAY"]) if (lists.has(key) && frequency !== "YEARLY") throw new RecurrenceInputError("syntax", `${key} requires FREQ=YEARLY.`);
	if (lists.has("BYMONTHDAY") && frequency !== "MONTHLY" && frequency !== "YEARLY") throw new RecurrenceInputError("syntax", "BYMONTHDAY requires FREQ=MONTHLY or FREQ=YEARLY.");
	if (ordinals > 0 && frequency !== "MONTHLY" && frequency !== "YEARLY") throw new RecurrenceInputError("syntax", "BYDAY ordinals require FREQ=MONTHLY or FREQ=YEARLY.");
	if (ordinals > 0 && lists.has("BYWEEKNO")) throw new RecurrenceInputError("syntax", "BYDAY ordinals cannot be combined with BYWEEKNO.");
	if (lists.has("BYSETPOS") && lists.size === 1 && byDay === void 0) throw new RecurrenceInputError("syntax", "BYSETPOS requires another BY rule part.");
}
/** Return an upper bound on BYDAY days in one month or year, or Infinity without BYDAY. */
function byDayDays(byDay, perWeekday) {
	return byDay === void 0 ? Infinity : byDay.ordinals + perWeekday * (byDay.entries - byDay.ordinals);
}
/**
* Return an upper bound on the days that one FREQ period selects; the rule carries its DTSTART defaults explicitly.
*
* One BYWEEKNO value selects at most 9 days of a year, because the year can also contain days of the adjacent year's
* week with that number.
*/
function periodDays(values) {
	const { lists, byDay } = values;
	const size = (key) => lists.get(key)?.length ?? Infinity;
	const months = lists.get("BYMONTH")?.length;
	switch (values.frequency) {
		case "MINUTELY":
		case "HOURLY":
		case "DAILY": return 1;
		case "WEEKLY": return Math.min(7, byDayDays(byDay, 1));
		case "MONTHLY": return Math.min(31, size("BYMONTHDAY"), byDayDays(byDay, 5));
		case "YEARLY":
			if (months === void 0) return Math.min(366, size("BYYEARDAY"), 12 * size("BYMONTHDAY"), 9 * size("BYWEEKNO"), byDayDays(byDay, 53));
			return Math.min(31 * months, size("BYYEARDAY"), months * size("BYMONTHDAY"), 9 * size("BYWEEKNO"), months * byDayDays(byDay, 5));
	}
}
/**
* Reject a rule whose FREQ period can generate more occurrences than one enumeration may visit.
* @param values - Rule values with the DTSTART defaults explicit.
*/
function checkCost(values, rejectIneffectiveSetPositions) {
	const times = [
		"BYHOUR",
		"BYMINUTE",
		"BYSECOND"
	].reduce((product, key) => product * (values.lists.get(key)?.length ?? 1), 1);
	const candidates = periodDays(values) * times;
	if (candidates > MAX_PERIOD_CANDIDATES) throw new RecurrenceInputError("density", `One FREQ period of this RRULE can contain up to ${candidates} occurrences; the limit is ${MAX_PERIOD_CANDIDATES}.`);
	const positions = values.lists.get("BYSETPOS") ?? [];
	if (rejectIneffectiveSetPositions && candidates === 1 && positions.length > 0) throw new RecurrenceInputError("syntax", `BYSETPOS has no effect because one FREQ=${values.frequency} period of this RRULE contains at most 1 occurrence. BYSETPOS selects among the occurrences of one period; for example, FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1 selects the last weekday of each month.`);
	if (positions.length > MAX_SET_POSITIONS) throw new RecurrenceInputError("syntax", `BYSETPOS must contain at most ${MAX_SET_POSITIONS} values.`);
	if (positions.some((position) => Math.abs(position) > candidates)) throw new RecurrenceInputError("syntax", `BYSETPOS values must be from 1 to ${candidates} or from -${candidates} to -1; one FREQ period of this RRULE contains at most ${candidates} occurrences.`);
}
/** Return the parts that the recurrence library derives from DTSTART when a DAILY or longer rule omits them. */
function startDefaults(values, start) {
	const defaults = /* @__PURE__ */ new Map();
	const { frequency, lists, byDay } = values;
	if (frequency === "MINUTELY" || frequency === "HOURLY") return defaults;
	const date = new Date(start);
	if (!lists.has("BYHOUR")) defaults.set("BYHOUR", String(date.getUTCHours()));
	if (!lists.has("BYMINUTE")) defaults.set("BYMINUTE", String(date.getUTCMinutes()));
	if (!lists.has("BYSECOND")) defaults.set("BYSECOND", String(date.getUTCSeconds()));
	if (byDay !== void 0 || lists.has("BYMONTHDAY") || lists.has("BYYEARDAY") || lists.has("BYWEEKNO")) return defaults;
	if (frequency === "WEEKLY") defaults.set("BYDAY", new Weekday((date.getUTCDay() + 6) % 7).toString());
	if (frequency === "MONTHLY" || frequency === "YEARLY") defaults.set("BYMONTHDAY", String(date.getUTCDate()));
	if (frequency === "YEARLY" && !lists.has("BYMONTH")) defaults.set("BYMONTH", String(date.getUTCMonth() + 1));
	return defaults;
}
/** Return the greatest common divisor of two positive safe integers. */
function gcd(left, right) {
	return right === 0 ? left : gcd(right, left % right);
}
/**
* Validate a rule completely, then derive its enumeration inputs.
* @param value - DTSTART and RRULE content.
*/
function prepareRRule(value, rejectIneffectiveSetPositions) {
	const [startLine, ruleLine] = recurrenceLines(value);
	const { timeZone, start } = parseStart(startLine);
	if (instantOf(new Date(start), timeZone) === void 0) throw new RecurrenceInputError("syntax", "DTSTART must be a real local date and time in its time zone.");
	const parts = ruleParts(ruleLine);
	const values = ruleValues(parts);
	checkCombination(values);
	const recurrenceParts = new Map([...parts, ...startDefaults(values, start)]);
	const recurrenceValues = ruleValues(recurrenceParts);
	checkCost(recurrenceValues, rejectIneffectiveSetPositions);
	recurrenceParts.delete("COUNT");
	recurrenceParts.delete("UNTIL");
	for (const key of TIME_PARTS) {
		const list = recurrenceValues.lists.get(key);
		if (list !== void 0) recurrenceParts.set(key, list.toSorted((left, right) => left - right).join(","));
	}
	const recurrence = [...recurrenceParts].map(([key, text]) => `${key}=${text}`).join(";");
	return {
		normalized: `${startLine}\n${ruleLine}`,
		timeZone,
		start,
		frequency: values.frequency,
		interval: values.interval,
		weekStart: values.weekStart,
		count: values.count,
		limit: values.until ?? MAX_FOUR_DIGIT_YEAR_MS,
		recurrence: `RRULE:${recurrence}`,
		invalidRunLimit: CYCLE_MS * values.interval / gcd(values.interval, CYCLE_PERIODS[values.frequency])
	};
}
/** Express an actual instant as UTC-shaped wall time in a zone. */
function wallOf(epoch, timeZone) {
	return epoch + Temporal.Instant.fromEpochMilliseconds(epoch).toZonedDateTimeISO(timeZone).offsetNanoseconds / 1e6;
}
/** Resolve a UTC-shaped wall time to its earlier actual instant, or undefined inside a forward transition. */
function instantOf(date, timeZone) {
	return resolveLocalInstant(new Temporal.PlainDateTime(date.getUTCFullYear(), date.getUTCMonth() + 1, date.getUTCDate(), date.getUTCHours(), date.getUTCMinutes(), date.getUTCSeconds()), timeZone);
}
/** Format UTC-shaped epoch milliseconds as a basic date-time without its Z. */
function basic(epoch) {
	return new Date(epoch).toISOString().replaceAll("-", "").replaceAll(":", "").slice(0, 15);
}
/** Return the epoch day of UTC-shaped epoch milliseconds. */
function dayOf(epoch) {
	return Math.floor(epoch / DAY_MS);
}
/** Return the epoch day of the week start at or before an epoch day; epoch day 0 is a Thursday. */
function weekStartDay(day, weekStart) {
	return day - ((day + 3 - weekStart) % 7 + 7) % 7;
}
/** Return the absolute month number of UTC-shaped epoch milliseconds. */
function monthOf(epoch) {
	const date = new Date(epoch);
	return date.getUTCFullYear() * 12 + date.getUTCMonth();
}
/** Return the year of UTC-shaped epoch milliseconds. */
function yearOf(epoch) {
	return new Date(epoch).getUTCFullYear();
}
/** Return the latest point `origin + k * step` at or before value. */
function floorStep(origin, value, step) {
	return origin + Math.floor((value - origin) / step) * step;
}
/** Return the start of the INTERVAL-aligned FREQ period that contains a wall time at or after DTSTART. */
function periodStart(prepared, wall) {
	const { start, interval, weekStart } = prepared;
	switch (prepared.frequency) {
		case "MINUTELY": return floorStep(start, wall, interval * 6e4);
		case "HOURLY": return floorStep(start, wall, interval * 36e5);
		case "DAILY": return floorStep(dayOf(start), dayOf(wall), interval) * DAY_MS;
		case "WEEKLY": return floorStep(weekStartDay(dayOf(start), weekStart), weekStartDay(dayOf(wall), weekStart), 7 * interval) * DAY_MS;
		case "MONTHLY": {
			const month = floorStep(monthOf(start), monthOf(wall), interval);
			return Date.UTC(Math.floor(month / 12), month % 12, 1);
		}
		case "YEARLY": return Date.UTC(floorStep(yearOf(start), yearOf(wall), interval), 0, 1);
	}
}
/**
* Return the enumeration start for the period that contains a wall time.
*
* The first period keeps the original DTSTART, because the library truncates a first WEEKLY period at DTSTART.
*/
function periodAnchor(prepared, wall) {
	return wall > prepared.start ? Math.max(periodStart(prepared, wall), prepared.start) : prepared.start;
}
/** Build the library rule that enumerates UTC-shaped wall times from a period start. */
function libraryRule(prepared, anchor) {
	return rrulestr(`DTSTART:${basic(anchor)}Z\n${prepared.recurrence}`);
}
/**
* Report whether the rule can generate a wall time.
*
* Whole 400-year cycles preserve the Gregorian calendar, so the scan starts at the latest equivalent period that leaves one
* complete calendar and INTERVAL cycle before year 9999. A rule without a wall time therefore costs at most two cycles.
*/
function hasCandidate(prepared) {
	if (prepared.frequency === "MINUTELY" || prepared.frequency === "HOURLY") return true;
	const first = periodStart(prepared, prepared.start);
	const firstYear = yearOf(first);
	const years = CYCLE_PERIODS.YEARLY;
	const cycleYears = years * prepared.interval / gcd(prepared.interval, CYCLE_PERIODS[prepared.frequency]);
	const anchor = first + Math.max(firstYear < MIN_RRULE_YEAR ? 1 : 0, Math.floor((9999 - firstYear - cycleYears - 1) / years)) * CYCLE_MS;
	return libraryRule(prepared, anchor).after(new Date(anchor), true) !== null;
}
/**
* Find the latest occurrence at or before a decision among wall times from one start, and the first later occurrence.
*
* Wall times at or before the decision wall time convert to instants only from the latest one backwards.
*/
function scanWindow(prepared, fromWall, decision) {
	const window = {
		latest: void 0,
		next: void 0
	};
	if (fromWall > LAST_RRULE_WALL_MS) return window;
	const bound = Math.min(decision, prepared.limit);
	const boundWall = wallOf(bound, prepared.timeZone);
	const below = [];
	const run = { lastValidWall: boundWall };
	libraryRule(prepared, periodAnchor(prepared, fromWall)).all((date) => {
		const wall = date.getTime();
		if (wall < fromWall) return true;
		if (wall <= boundWall) {
			below.push(date);
			return true;
		}
		if (wall >= prepared.limit + DAY_MS) return false;
		const instant = instantOf(date, prepared.timeZone);
		if (instant === void 0) return wall - run.lastValidWall <= prepared.invalidRunLimit;
		run.lastValidWall = wall;
		if (instant <= bound) {
			window.latest = instant;
			return true;
		}
		if (instant <= prepared.limit) window.next = instant;
		return false;
	});
	while (window.latest === void 0) {
		const date = below.pop();
		if (date === void 0) break;
		const instant = instantOf(date, prepared.timeZone);
		if (instant !== void 0 && instant <= bound) window.latest = instant;
	}
	return window;
}
/** Find the latest due occurrence; the window starts two days before the decision and doubles while it has none. */
function dispatchWindow(prepared, saved, decision) {
	const savedWall = wallOf(saved, prepared.timeZone);
	const boundWall = wallOf(Math.min(decision, prepared.limit), prepared.timeZone);
	for (let span = 2 * DAY_MS;; span *= 2) {
		const fromWall = Math.max(boundWall - span, savedWall);
		const window = scanWindow(prepared, fromWall, decision);
		if (window.latest !== void 0 || fromWall === savedWall) return window;
	}
}
/** Enumerate distinct COUNT instances from DTSTART through the first occurrence after a decision. */
function countWindow(prepared, decision, count) {
	const window = {
		latest: void 0,
		next: void 0
	};
	const run = {
		remaining: count,
		lastValidWall: prepared.start
	};
	libraryRule(prepared, prepared.start).all((date) => {
		const wall = date.getTime();
		const instant = instantOf(date, prepared.timeZone);
		if (instant === void 0) return wall - run.lastValidWall <= prepared.invalidRunLimit;
		if (instant === window.latest) return true;
		run.lastValidWall = wall;
		if (instant > decision) {
			if (instant <= MAX_FOUR_DIGIT_YEAR_MS) window.next = instant;
			return false;
		}
		window.latest = instant;
		run.remaining -= 1;
		return run.remaining > 0;
	});
	return window;
}
/** Format the first occurrence strictly after a representable reference time. */
function nextOccurrence(prepared, after) {
	if (!hasCandidate(prepared)) return void 0;
	const next = prepared.count === void 0 ? scanWindow(prepared, wallOf(after, prepared.timeZone), after).next : countWindow(prepared, after, prepared.count).next;
	return next === void 0 ? void 0 : new Date(next).toISOString();
}
/** Parsed two-line DTSTART/RRULE value with bounded occurrence arithmetic. */
var RRule = class RRule {
	prepared;
	constructor(prepared) {
		this.prepared = prepared;
	}
	/** Trimmed two-line expression; DTSTART owns the zone and recurrence anchor. */
	get expression() {
		return this.prepared.normalized;
	}
	/** Canonical zone from DTSTART, or UTC for a Z start. */
	get timeZone() {
		return this.prepared.timeZone;
	}
	/**
	* Validate syntax and enumeration cost before any occurrence search.
	* @param expression - Exactly one DTSTART line followed by one RRULE line.
	* @param options - Optional stricter BYSETPOS authoring checks.
	* @returns A bounded parsed value, or a syntax, zone, or density refusal.
	*/
	static parse(expression, options = {}) {
		return parseRecurrence(() => {
			const rule = new RRule(prepareRRule(expression, options.rejectIneffectiveSetPositions ?? false));
			Object.freeze(rule);
			return rule;
		});
	}
	/**
	* First occurrence strictly after the explicit lower bound, or exhaustion.
	* @param after - Representable epoch milliseconds that the occurrence must exceed.
	* @returns The UTC occurrence, or exhaustion of the accepted rule.
	*/
	next(after) {
		if (!isFourDigitYearInstant(after)) throw new RangeError("RRULE lower bound must be a four-digit-year instant");
		return nextOccurrence(this.prepared, after);
	}
	/**
	* Latest due occurrence and first future occurrence, retaining the committed floor.
	* @param scheduledAt - Canonical four-digit-year UTC occurrence with milliseconds, retained across zone-data changes.
	* @param acceptedAt - Representable decision time at or after that occurrence.
	* @returns The latest due occurrence and optional future successor.
	* @throws RangeError when either instant is invalid or the decision precedes the committed occurrence.
	*/
	resolve(scheduledAt, acceptedAt) {
		const savedTarget = recurrenceDecision(scheduledAt, acceptedAt);
		const prepared = this.prepared;
		const window = prepared.count === void 0 ? dispatchWindow(prepared, savedTarget, acceptedAt) : countWindow(prepared, acceptedAt, prepared.count);
		const occurrenceAt = new Date(Math.max(window.latest ?? savedTarget, savedTarget)).toISOString();
		const nextScheduledAt = window.next === void 0 ? void 0 : new Date(window.next).toISOString();
		return Object.freeze(nextScheduledAt === void 0 ? { occurrenceAt } : {
			occurrenceAt,
			nextScheduledAt
		});
	}
};
//#endregion
//#region lib/types/index.js
/**
* Time vocabulary shared by the boundaries that accept a caller's zone or a
* wall-clock selector, elapsed span, or bounded Cron/RRULE recurrence.
*
* The library validates, canonicalizes, parses, and renders one canonical instant
* format; it produces no model-facing text and owns no failure vocabulary. A refusal that carries a
* reason is a discriminated result, and each caller maps that reason onto its
* own domain code and diagnostic. `canonicalClientTimeZone` is the wire-boundary
* form and answers a name or `undefined`; `formatFourDigitYearInstant` and
* `resolveLocalInstant` take already-validated values and answer a string or
* epoch milliseconds directly; `Duration` holds a span in milliseconds with no
* implicit numeric conversion. Cron and RRule parse distinct recurrence types
* and compute occurrences from explicit times without task state or timers.
* @module @deepseek-ai/dsh-util-time
*/
/**
* Validate and canonicalize one caller-supplied IANA zone at a wire boundary.
*
* The canonical name is what a later reader needs: a zone identity is stored on
* durable records and resolved again by another process, so an alias accepted
* here would not compare equal to the zone a reader derives.
* @param value - the caller's reported zone name.
* @returns the canonical zone, or `undefined` when the name is unusable.
*/
function canonicalClientTimeZone(value) {
	const resolved = canonicalizeTimeZone(value);
	return resolved.kind === "accepted" ? resolved.timeZone : void 0;
}
//#endregion
export { CRON_SEARCH_HORIZON_YEARS, Cron, Duration, MAX_FOUR_DIGIT_YEAR_MS, MIN_FOUR_DIGIT_YEAR_MS, RRule, canonicalClientTimeZone, canonicalizeTimeZone, formatFourDigitYearInstant, fourDigitYearEpoch, isFourDigitYearInstant, parseLocalCalendar, parseOffsetInstant, resolveLocalInstant };
