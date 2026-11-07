/**
 * Host task decoding, timing validation, occurrence selection, and reminder framing.
 * @module @deepseek-ai/dsh-schedule
 */
import { Temporal } from '@js-temporal/polyfill';
import type { AfterScheduleRecord, AtInput, AtScheduleRecord, EveryScheduleRecord, CronInput, CronScheduleRecord, DailyInput, DailyScheduleRecord, WeeklyInput, WeeklyScheduleRecord, RecurringScheduleRecord, ScheduleId as ScheduleIdType, ScheduleInputErrorCode } from './types.ts';
import type { ScheduleRecord, RRuleScheduleRecord } from './types.ts';
/** Minimum supported fixed-rate interval. */
export declare const MIN_EVERY_INTERVAL_SECONDS = 60;
/** Maximum stored task title length. */
export declare const MAX_TITLE_LENGTH = 120;
/** Error from malformed or transition-invalid durable Schedule data. */
export declare class ScheduleLogError extends Error {
    /** Stable machine-readable error code. */
    readonly code: "corrupt_schedule_log";
    /**
     * Construct a durable-log failure.
     * @param message - Package-specific violated invariant.
     */
    constructor(message: string);
}
/** Error from a model-supplied Schedule rule or target Session that cannot become a record. */
export declare class ScheduleInputError extends Error {
    /** Stable public Schedule input code. */
    readonly code: ScheduleInputErrorCode;
    /**
     * Construct a stable input failure.
     * @param code - Public Schedule error discriminator.
     * @param message - Stable public diagnostic.
     * @param options - Optional contained implementation cause.
     */
    constructor(code: ScheduleInputErrorCode, message: string, options?: ErrorOptions);
}
/** One latest-only recurring decision derived without enumerating a backlog. */
export interface RecurringOccurrence {
    /** Latest occurrence due at the decision time. */
    readonly occurrenceAt: string;
    /** First eligible target after the decision, or exhaustion. */
    readonly nextScheduledAt?: string;
}
/**
 * Brand a raw task id without changing its runtime value.
 * @param value - Raw task id.
 * @returns The same string with the Schedule brand.
 */
export declare function ScheduleId(value: string): ScheduleIdType;
/**
 * Narrow an unknown value to a non-array object.
 * @param value - Value to inspect.
 * @returns Whether the value is a non-null, non-array object.
 */
export declare function isRecord(value: unknown): value is Record<string, unknown>;
/**
 * Require exactly the named durable object keys.
 * @param value - Object to inspect.
 * @param expected - Complete allowed key set.
 * @returns Whether every expected key is present and no other key is present.
 */
export declare function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean;
/** Stable diagnostic for a title that is missing or empty after trimming. */
export declare const REQUIRED_TITLE_MESSAGE = "title is required and must be non-empty after trimming.";
/**
 * Validate the title supplied at creation.
 *
 * Creation requires an explicit title: a missing, blank-after-trim, or over-long
 * value throws instead of deriving a name from the instruction.
 * @param title - Task name supplied at creation.
 * @returns The trimmed title; an invalid title throws ScheduleInputError.
 */
export declare function scheduleTitle(title: string): string;
/**
 * Validate one required stored title at the durable boundary.
 *
 * Only records written after titles became required are read: a missing,
 * blank-after-trim, untrimmed, or over-long stored title is invalid, and no name
 * is derived from the instruction.
 * @param value - Untrusted durable title field.
 * @returns The stored title; an invalid title throws ScheduleLogError.
 */
export declare function decodeStoredTitle(value: unknown): string;
/**
 * Validate one stable task id at the durable boundary.
 * @param value - Untrusted durable id field.
 * @returns The branded id; an invalid id throws ScheduleLogError.
 */
export declare function decodeId(value: unknown): ScheduleIdType;
/**
 * Validate one canonical four-digit-year UTC instant at the durable boundary.
 * @param value - Untrusted durable instant field.
 * @returns The canonical instant; an invalid instant throws ScheduleLogError.
 */
export declare function decodeInstant(value: unknown): string;
/**
 * Validate and canonicalize one raw IANA time-zone selector.
 * @param value - Candidate `UTC` or IANA Area/Location name.
 * @returns The runtime's canonical IANA name.
 */
export declare function canonicalizeTimeZone(value: string): string;
/**
 * Require a safe, representable, strictly future UTC target.
 * @param epoch - Candidate target in epoch milliseconds.
 * @param now - Exclusive lower bound in epoch milliseconds.
 * @returns The canonical UTC instant; an out-of-range or past target throws ScheduleInputError.
 */
export declare function futureInstant(epoch: number, now: number): string;
/**
 * Parse one strict local clock time accepted by the weekly selector.
 * @param value - Candidate `HH:mm:ss` time with optional one-to-three fractional digits.
 * @returns The parsed plain time; malformed input throws ScheduleInputError.
 */
export declare function weeklyTime(value: string): Temporal.PlainTime;
/**
 * Normalize the explicit ISO weekday set of one weekly rule.
 * @param weekdays - Untrusted candidate weekday values.
 * @returns Frozen unique ascending weekdays from 1 (Monday) through 7 (Sunday).
 */
export declare function normalizeWeekdays(weekdays: unknown): number[];
/**
 * Decode one of the seven current Host timing records without recomputing its committed occurrence.
 * @param value - Untrusted durable JSON record.
 * @returns Detached frozen record with its required title and authoritative timing fields.
 */
export declare function decodeScheduleRecord(value: unknown): ScheduleRecord;
/** Timing fields one fixed-rate decision needs from its record. */
type EveryOccurrenceInput = Pick<EveryScheduleRecord, 'everySeconds' | 'scheduledAt'>;
/**
 * Resolve one fixed-rate decision without enumerating missed occurrences.
 * @param record - Active record whose target is the earliest unaccepted occurrence.
 * @param acceptedAt - Wall-clock decision time in epoch milliseconds.
 * @returns The latest due occurrence and first strictly future target, if representable.
 */
export declare function resolveEveryOccurrence(record: EveryOccurrenceInput, acceptedAt: number): RecurringOccurrence;
/**
 * Resolve a wall-clock rule only when its decision is at a representable four-digit-year instant.
 * @param selector - Rule kind named in the failure message.
 * @param acceptedAt - Explicit wall-clock decision time in epoch milliseconds.
 * @returns The validated decision time.
 */
export declare function acceptedDecision(selector: string, acceptedAt: number): number;
/**
 * Resolve a daily decision near the decision's local date, not across its missed history.
 * @param record - Daily rule with a committed earliest unaccepted UTC target.
 * @param acceptedAt - Explicit wall-clock decision time, at or after the committed target.
 * @returns Latest actual due occurrence and the next future occurrence on a later local date.
 */
export declare function resolveDailyOccurrence(record: DailyScheduleRecord, acceptedAt: number): RecurringOccurrence;
/**
 * Resolve a weekly decision near the decision's local date, not across its missed history.
 * @param record - Weekly rule with a committed earliest unaccepted UTC target.
 * @param acceptedAt - Explicit wall-clock decision time, at or after the committed target.
 * @returns Latest actual due occurrence and the next future occurrence on a selected weekday.
 */
export declare function resolveWeeklyOccurrence(record: WeeklyScheduleRecord, acceptedAt: number): RecurringOccurrence;
/**
 * Resolve a Cron rule through the shared bounded recurrence engine.
 * @param record - Authoritative Cron expression, zone, and committed occurrence.
 * @param acceptedAt - Decision time at or after the committed occurrence.
 * @returns The latest due occurrence and optional future successor.
 */
export declare function resolveCronOccurrence(record: CronScheduleRecord, acceptedAt: number): RecurringOccurrence;
/**
 * Identify recurring Host records explicitly, excluding both one-shot variants.
 * @param record - Current Host schedule record.
 * @returns Whether the record uses a recurring rule.
 */
export declare function isRecurringScheduleRecord(record: ScheduleRecord): record is RecurringScheduleRecord;
/**
 * Resolve one due recurring Host record with its rule-specific calendar or interval arithmetic.
 * @param record - Due recurring rule.
 * @param acceptedAt - Explicit decision time in epoch milliseconds.
 * @returns Latest due occurrence and optional future target.
 */
export declare function resolveRecurringOccurrence(record: RecurringScheduleRecord, acceptedAt: number): RecurringOccurrence;
/**
 * Validate a model after rule and compute its durable target.
 * @param id - Already allocated task id.
 * @param prompt - Reminder content supplied at creation.
 * @param afterSeconds - Requested positive delay.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen durable after record.
 */
export declare function createAfterScheduleRecord(id: ScheduleIdType, prompt: string, afterSeconds: number, now: number, title: string): AfterScheduleRecord;
/**
 * Validate an absolute selector and compute its sole durable UTC target.
 * @param id - Already allocated task id.
 * @param prompt - Reminder content supplied at creation.
 * @param at - Explicit-offset instant or structured local calendar value.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen durable absolute one-shot record.
 */
export declare function createAtScheduleRecord(id: ScheduleIdType, prompt: string, at: AtInput, now: number, title: string): AtScheduleRecord;
/**
 * Parse an absolute selector without requiring it to be future.
 * @param at - Explicit-offset instant or strict local calendar input.
 * @returns Resolved epoch milliseconds; malformed input throws ScheduleInputError.
 */
export declare function parseAtInput(at: AtInput): number;
/**
 * Validate a fixed-rate selector and compute the first target of a new interval anchor.
 * @param id - Already allocated task id.
 * @param prompt - Reminder content supplied at creation.
 * @param everySeconds - Requested fixed safe-integer interval.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen durable fixed-rate record.
 */
export declare function createEveryScheduleRecord(id: ScheduleIdType, prompt: string, everySeconds: number, now: number, title: string): EveryScheduleRecord;
/**
 * Create a daily wall-clock rule with a strictly future committed UTC target.
 * @param id - Already allocated task identity.
 * @param prompt - Reminder content supplied at creation.
 * @param daily - Strict local time and explicit IANA zone.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen daily record; absent future dates throw time_out_of_range.
 */
export declare function createDailyScheduleRecord(id: ScheduleIdType, prompt: string, daily: DailyInput, now: number, title: string): DailyScheduleRecord;
/**
 * Normalize a daily selector without calculating a new committed target.
 * @param daily - Strict local time and explicit IANA zone.
 * @returns Normalized time and canonical zone; malformed input throws ScheduleInputError.
 */
export declare function parseDailyInput(daily: DailyInput): {
    readonly time: string;
    readonly timeZone: string;
};
/**
 * Create a weekly wall-clock rule with a strictly future committed UTC target.
 * @param id - Already allocated task identity.
 * @param prompt - Reminder content supplied at creation.
 * @param weekly - Strict local time, explicit IANA zone, and explicit ISO weekday set.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen weekly record; absent future weekdays throw time_out_of_range.
 */
export declare function createWeeklyScheduleRecord(id: ScheduleIdType, prompt: string, weekly: WeeklyInput, now: number, title: string): WeeklyScheduleRecord;
/**
 * Create a cron wall-clock rule with a strictly future committed UTC target.
 * @param id - Already allocated task identity.
 * @param prompt - Reminder content supplied at creation.
 * @param cron - Strict five-field expression and explicit IANA zone.
 * @param now - Single rule-acceptance wall-clock sample in epoch milliseconds.
 * @param title - Required task name supplied at creation.
 * @returns Frozen cron record; absent future occurrences throw time_out_of_range.
 */
export declare function createCronScheduleRecord(id: ScheduleIdType, prompt: string, cron: CronInput, now: number, title: string): CronScheduleRecord;
/**
 * Normalize a weekly selector without calculating a new committed target.
 * @param weekly - Strict local time, explicit IANA zone, and explicit ISO weekday set.
 * @returns Normalized time, canonical zone, and unique ascending weekdays; malformed input throws ScheduleInputError.
 */
export declare function parseWeeklyInput(weekly: WeeklyInput): {
    readonly time: string;
    readonly timeZone: string;
    readonly weekdays: number[];
};
/**
 * Canonicalize one supported five-field Cron expression.
 * @param expression - Strict five-field input text.
 * @returns Canonical text preserving the Vixie day-matching semantics.
 */
export declare function canonicalizeCronExpression(expression: string): string;
/**
 * Normalize a cron selector without calculating a new committed target.
 * @param cron - Strict five-field expression and explicit IANA zone.
 * @returns The canonical expression and canonical zone; malformed input throws ScheduleInputError.
 */
export declare function parseCronInput(cron: CronInput): {
    readonly expression: string;
    readonly timeZone: string;
};
/**
 * Render the fixed model framing for one due reminder.
 * @param record - Due task identity, instruction, and delivered occurrence.
 * @returns Stable model-visible text with JSON-escaped dynamic fields.
 */
export declare function renderReminderFraming(record: {
    readonly id: ScheduleIdType;
    readonly prompt: string;
    readonly scheduledAt: string;
}): string;
/**
 * Render one recurring reminder batch in the supplied order.
 * @param reminders - Complete admitted batch with one latest occurrence per record.
 * @returns Stable model-visible text whose dynamic payload is canonical JSON.
 */
export declare function renderRecurringReminderBatchFraming(reminders: readonly {
    readonly record: {
        readonly id: ScheduleIdType;
        readonly prompt: string;
    };
    readonly occurrenceAt: string;
}[]): string;
/**
 * Validate and normalize the supported RFC 5545 recurrence content.
 * @param value - Two-line DTSTART and RRULE input.
 * @returns The trimmed newline-separated recurrence; an unsupported rule throws ScheduleInputError.
 */
export declare function normalizeRRule(value: string): string;
/**
 * Create one Host task record with its first committed occurrence.
 * @param id - Host-allocated canonical schedule id.
 * @param title - User-facing schedule title.
 * @param prompt - Instruction delivered when the schedule fires.
 * @param rrule - DTSTART and RRULE content.
 * @param now - Exclusive creation-time lower bound in epoch milliseconds.
 * @returns Frozen schedule record.
 */
export declare function createRRuleScheduleRecord(id: ScheduleIdType, title: string, prompt: string, rrule: string, now: number): RRuleScheduleRecord;
/**
 * Update descriptive fields and optionally recompute the next occurrence.
 * @param current - Existing schedule record.
 * @param change - Supplied editable record fields.
 * @param now - Exclusive edit-time lower bound in epoch milliseconds.
 * @param recompute - Whether to derive a fresh next occurrence.
 * @returns Frozen updated schedule record.
 */
export declare function updateRRuleScheduleRecord(current: RRuleScheduleRecord, change: {
    readonly title?: string;
    readonly prompt?: string;
    readonly rrule?: string;
}, now: number, recompute: boolean): RRuleScheduleRecord;
/**
 * Resolve the latest due occurrence and the first later occurrence without replaying a backlog.
 * @param record - Due schedule record.
 * @param acceptedAt - Dispatch admission time in epoch milliseconds.
 * @returns Latest due occurrence and optional next occurrence.
 */
export declare function resolveRRuleOccurrence(record: RRuleScheduleRecord, acceptedAt: number): RecurringOccurrence;
/**
 * Decode a current Host task record.
 * @param value - Untrusted durable JSON record.
 * @returns Detached frozen record; malformed fields throw ScheduleLogError.
 */
export declare function decodeRRuleScheduleRecord(value: unknown): RRuleScheduleRecord;
/**
 * Render an instant in the authoritative zone of any timing kind; elapsed and absolute rules use UTC.
 * @param record - Timing record that owns the display zone.
 * @param instant - UTC instant to render with second precision.
 * @returns Local ISO date-time with its numeric offset.
 */
export declare function formatScheduleLocalTime(record: ScheduleRecord, instant: string): string;
export {};
//# sourceMappingURL=domain.d.ts.map