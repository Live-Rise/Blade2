import type { RecurrenceOccurrence, RecurrenceResult } from './recurrence.ts';
/** Parsed two-line DTSTART/RRULE value with bounded occurrence arithmetic. */
export declare class RRule {
    private readonly prepared;
    private constructor();
    /** Trimmed two-line expression; DTSTART owns the zone and recurrence anchor. */
    get expression(): string;
    /** Canonical zone from DTSTART, or UTC for a Z start. */
    get timeZone(): string;
    /**
     * Validate syntax and enumeration cost before any occurrence search.
     * @param expression - Exactly one DTSTART line followed by one RRULE line.
     * @param options - Optional stricter BYSETPOS authoring checks.
     * @returns A bounded parsed value, or a syntax, zone, or density refusal.
     */
    static parse(expression: string, options?: {
        readonly rejectIneffectiveSetPositions?: boolean;
    }): RecurrenceResult<RRule>;
    /**
     * First occurrence strictly after the explicit lower bound, or exhaustion.
     * @param after - Representable epoch milliseconds that the occurrence must exceed.
     * @returns The UTC occurrence, or exhaustion of the accepted rule.
     */
    next(after: number): string | undefined;
    /**
     * Latest due occurrence and first future occurrence, retaining the committed floor.
     * @param scheduledAt - Canonical four-digit-year UTC occurrence with milliseconds, retained across zone-data changes.
     * @param acceptedAt - Representable decision time at or after that occurrence.
     * @returns The latest due occurrence and optional future successor.
     * @throws RangeError when either instant is invalid or the decision precedes the committed occurrence.
     */
    resolve(scheduledAt: string, acceptedAt: number): RecurrenceOccurrence;
}
//# sourceMappingURL=rrule.d.ts.map