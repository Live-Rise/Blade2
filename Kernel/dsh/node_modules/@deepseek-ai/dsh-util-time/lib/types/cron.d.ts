import type { RecurrenceOccurrence, RecurrenceResult } from './recurrence.ts';
/** Gregorian calendar repetition bounds both forward and backward searches. */
export declare const CRON_SEARCH_HORIZON_YEARS = 400;
/** Parsed five-field cron rule: canonical text plus every field's matched values. */
export interface CronFields {
    /** Canonical expression stored in the durable record. */
    readonly expression: string;
    /** Unique ascending matched minutes. */
    readonly minutes: readonly number[];
    /** Unique ascending matched hours. */
    readonly hours: readonly number[];
    /** Unique ascending matched days of the month. */
    readonly daysOfMonth: readonly number[];
    /** Unique ascending matched months. */
    readonly months: readonly number[];
    /** Unique ascending matched cron weekdays, Sunday 0 through Saturday 6. */
    readonly daysOfWeek: readonly number[];
    /** Whether the day-of-month field text starts with `*` (Vixie `DOM_STAR`). */
    readonly dayOfMonthStar: boolean;
    /** Whether the day-of-week field text starts with `*` (Vixie `DOW_STAR`). */
    readonly dayOfWeekStar: boolean;
}
/** Parsed Cron value; expression and zone stay authoritative independently of any task. */
export declare class Cron {
    private readonly parsed;
    readonly timeZone: string;
    private constructor();
    /** Immutable matched fields and canonical text for consumers such as rule descriptions. */
    get fields(): CronFields;
    /** Canonical five-field expression, preserving Vixie star semantics. */
    get expression(): string;
    /**
     * Parse one five-field expression and its explicit IANA zone.
     * @param expression - Five numeric Cron fields with Vixie day matching.
     * @param timeZone - UTC or an IANA Area/Location zone.
     * @returns A parsed value, or a syntax/zone refusal without task policy.
     */
    static parse(expression: string, timeZone: string): RecurrenceResult<Cron>;
    /**
     * First strictly future occurrence, skipping gaps and choosing the earlier overlap.
     * @param after - Representable epoch milliseconds that the occurrence must exceed.
     * @returns The UTC occurrence, or exhaustion of the bounded search.
     */
    next(after: number): string | undefined;
    /**
     * Latest due occurrence and first future occurrence, without replaying a backlog.
     * @param scheduledAt - Canonical four-digit-year UTC occurrence with milliseconds, retained across zone-data changes.
     * @param acceptedAt - Representable decision time at or after that occurrence.
     * @returns The latest due occurrence and optional future successor.
     * @throws RangeError when either instant is invalid or the decision precedes the committed occurrence.
     */
    resolve(scheduledAt: string, acceptedAt: number): RecurrenceOccurrence;
}
//# sourceMappingURL=cron.d.ts.map