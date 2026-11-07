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
import { Temporal } from '@js-temporal/polyfill';
/** Inclusive lower epoch-millisecond bound of a four-digit-year RFC 3339 UTC instant. */
export declare const MIN_FOUR_DIGIT_YEAR_MS: number;
/** Inclusive upper epoch-millisecond bound of a four-digit-year RFC 3339 UTC instant. */
export declare const MAX_FOUR_DIGIT_YEAR_MS: number;
/** Exact calendar fields of one local wall-clock value. */
export interface LocalCalendarTime {
    /**
     * Year of the calendar date. `parseLocalCalendar` rejects year 0 in untrusted
     * text; `resolveLocalInstant` accepts any year a real `Temporal` date carries,
     * so a caller that needs the four-digit bound filters the answered epoch.
     */
    readonly year: number;
    /** Month from 1 through 12. */
    readonly month: number;
    /** Day of month from 1 through the month's length. */
    readonly day: number;
    /** Hour from 0 through 23. */
    readonly hour: number;
    /** Minute from 0 through 59. */
    readonly minute: number;
    /** Second from 0 through 59; leap seconds are rejected. */
    readonly second: number;
    /** Millisecond from 0 through 999. */
    readonly millisecond: number;
}
/** Why an RFC 3339 instant string is unusable. */
export type InstantRejection = 'not_rfc3339'
/** A field outside its calendar range: year 0, or an hour, minute, or second above its maximum. */
 | 'out_of_range' | 'not_a_calendar_date' | 'invalid_numeric_offset';
/** Why a local calendar selector is unusable. */
export type LocalCalendarRejection = 'malformed' | 'out_of_range' | 'not_a_calendar_date';
/** Why one IANA zone selector is unusable. */
export type TimeZoneRejection = 'blank' | 'padded' | 'not_area_location' | 'unsupported' | 'not_canonical_shape';
/** Discriminated result of parsing one RFC 3339 instant string. */
export type InstantResult = {
    readonly kind: 'accepted';
    readonly epoch: number;
} | {
    readonly kind: 'rejected';
    readonly reason: InstantRejection;
};
/** Discriminated result of parsing one local calendar selector. */
export type LocalCalendarResult = {
    readonly kind: 'accepted';
    readonly time: LocalCalendarTime;
} | {
    readonly kind: 'rejected';
    readonly reason: LocalCalendarRejection;
};
/** Discriminated result of resolving one raw IANA zone selector. */
export type TimeZoneResult = {
    readonly kind: 'accepted';
    readonly timeZone: string;
} | {
    readonly kind: 'rejected';
    readonly reason: TimeZoneRejection;
    /** Platform error caught while resolving the name; present when the platform refused a well-formed one. */
    readonly cause?: unknown;
};
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
export declare function parseLocalCalendar(date: string, time: string): LocalCalendarResult;
/**
 * Parse one RFC 3339 instant whose numeric offset is part of the input.
 *
 * A bare `Z` and an explicit numeric offset are both accepted, and the offset is
 * subtracted from the local wall-clock value. A negative zero offset, an offset
 * whose hour or minute is out of range, and a non-existent calendar date are refused.
 * @param value - Candidate instant text.
 * @returns The resolved epoch milliseconds, or the reason the text is unusable.
 */
export declare function parseOffsetInstant(value: string): InstantResult;
/**
 * Validate one canonical four-digit-year RFC 3339 UTC instant.
 *
 * The accepted profile carries exactly three fractional digits and a `Z`, and
 * every year from 0001 through 9999 whose calendar date exists is representable.
 * @param value - Untrusted candidate instant.
 * @returns The epoch milliseconds, or the reason the text is not the canonical profile.
 */
export declare function fourDigitYearEpoch(value: string): InstantResult;
/**
 * Whether an epoch is representable as a canonical four-digit-year RFC 3339 UTC instant.
 *
 * Callers that only need the bound use this instead of re-deriving it from the
 * two constants, so the range has one owner.
 * @param epoch - Candidate epoch milliseconds.
 * @returns Whether the epoch is a safe integer inside the four-digit-year range.
 */
export declare function isFourDigitYearInstant(epoch: number): boolean;
/**
 * Format one epoch as the canonical four-digit-year RFC 3339 UTC instant.
 *
 * The bound check runs first, so a caller that also compares the result with a
 * decision time never formats an unrepresentable instant.
 * @param epoch - Candidate epoch milliseconds; a non-integral value is unrepresentable.
 * @returns The canonical instant text, or `undefined` when the epoch is unrepresentable.
 */
export declare function formatFourDigitYearInstant(epoch: number): string | undefined;
/**
 * Validate and canonicalize one raw IANA zone selector.
 *
 * A zone identity is stored on durable records and resolved again by another
 * process, so an alias is answered by its platform-canonical name. Abbreviations,
 * numeric offsets, single-segment names, and unknown names are refused.
 * @param value - Candidate `UTC` or IANA Area/Location name.
 * @returns The canonical zone, or the reason the name is unusable.
 */
export declare function canonicalizeTimeZone(value: string): TimeZoneResult;
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
export declare function resolveLocalInstant(time: LocalCalendarTime | Temporal.PlainDateTime, timeZone: string): number | undefined;
//# sourceMappingURL=instant.d.ts.map