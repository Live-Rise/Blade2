/** Latest admitted occurrence and the first future occurrence, if one exists. */
export interface RecurrenceOccurrence {
    /** Latest due UTC occurrence, never before the committed lower bound. */
    readonly occurrenceAt: string;
    /** First strictly future UTC occurrence, omitted at exhaustion. */
    readonly nextScheduledAt?: string;
}
/** Why recurrence content cannot be evaluated by the bounded engine. */
export type RecurrenceRejection = 'syntax' | 'time_zone' | 'density';
/** A parsed recurrence or its validation reason; no consumer-specific error codes. */
export type RecurrenceResult<T> = {
    readonly kind: 'accepted';
    readonly rule: T;
} | {
    readonly kind: 'rejected';
    readonly reason: RecurrenceRejection;
    readonly detail: string;
    readonly cause?: unknown;
};
/** Internal validation failure, contained by the public parsers. */
export declare class RecurrenceInputError extends Error {
    readonly reason: RecurrenceRejection;
    constructor(reason: RecurrenceRejection, message: string, options?: ErrorOptions);
}
/**
 * Contain recurrence validation failures while preserving unexpected implementation errors.
 * @param parse - Synchronous rule constructor whose input failures use RecurrenceInputError.
 * @returns The parsed value or its validation refusal.
 */
export declare function parseRecurrence<T>(parse: () => T): RecurrenceResult<T>;
/**
 * Validate the explicit decision and its committed lower bound before calendar arithmetic.
 * @param scheduledAt - Committed canonical four-digit-year UTC occurrence with milliseconds.
 * @param acceptedAt - Explicit epoch milliseconds of the delivery decision.
 * @returns The committed epoch, or throws RangeError for an invalid decision.
 */
export declare function recurrenceDecision(scheduledAt: string, acceptedAt: number): number;
//# sourceMappingURL=recurrence.d.ts.map