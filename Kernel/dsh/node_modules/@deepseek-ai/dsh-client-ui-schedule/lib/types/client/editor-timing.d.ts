/** Lossless guided and Custom editor projections for typed Schedule timing. */
import type { ScheduleCatalogEntry, ScheduleTiming } from '@deepseek-ai/dsh-schedule/client';
/** Periods represented by guided controls, plus a lossless advanced editor. */
export type FrequencyPreset = 'once' | 'minutely' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'advanced';
/** Editable values that determine one typed timing selector. */
export interface TimingDraft {
    readonly frequency: FrequencyPreset;
    readonly interval: string;
    readonly start: string;
    readonly timeZone: string;
    readonly rrule: string;
    /** Stored typed timing, retained in Custom and used to anchor guided edits to the current period. */
    readonly retainedTiming?: ScheduleTiming;
    readonly weekdays: readonly number[];
    /** Day of the month; a month without that day runs on its last day. */
    readonly monthDay: number;
    /** Daily `HH:MM` clock time. */
    readonly dailyTime: string;
}
/**
 * Initial form values for a new schedule.
 * @returns Guided timing with a one-hour-ahead one-shot default.
 */
export declare function defaultTiming(): TimingDraft;
/**
 * Format guided timing as DTSTART and RRULE content lines; return advanced text verbatim.
 * Draft fields and future occurrences require separate validation.
 * @param draft - Guided timing values or advanced text.
 * @param intervalFromNow - Use the current minute for intervals and the current period for edited typed recurrences;
 * false preserves the draft anchor.
 * @returns RFC 5545 content lines.
 */
export declare function rruleFromDraft(draft: TimingDraft, intervalFromNow?: boolean): string;
/**
 * Recover only supported form rules; unknown rules remain verbatim in Advanced mode.
 * @param record - Stored schedule to edit.
 * @returns Guided values or the original advanced text.
 */
export declare function timingFromRecord(record: ScheduleCatalogEntry): TimingDraft;
/**
 * Resolve Custom Cron or RRULE text into its distinct management type.
 * @param draft - Current rule editor values, including any untouched stored timing.
 * @param intervalFromNow - Whether saves anchor intervals and edited typed recurrences to the current date and time.
 * @returns Typed timing for the shared Schedule management API.
 */
export declare function timingFromDraft(draft: TimingDraft, intervalFromNow?: boolean): ScheduleTiming;
//# sourceMappingURL=editor-timing.d.ts.map