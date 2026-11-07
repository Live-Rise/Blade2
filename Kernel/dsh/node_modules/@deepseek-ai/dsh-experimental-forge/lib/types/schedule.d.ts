/** Poll cadence for watched change requests. */
import type { ChangeRequestSnapshot, ForgeFailure } from './types.ts';
/** Resolved intervals that pace polling. */
export interface PollIntervals {
    /** Delay while checks run or mergeability is unknown. */
    readonly activeIntervalMs: number;
    /** Delay for a closed request or an open request whose checks have settled. */
    readonly idleIntervalMs: number;
    /** Largest delay after consecutive failures. */
    readonly maxBackoffMs: number;
}
/**
 * Choose the delay before the next poll of one change request. Failures back
 * off exponentially from the active interval; merged change requests stop
 * polling, while closed ones use the idle cadence to detect reopening.
 * @param result - latest snapshot or failure.
 * @param failures - consecutive failures, counting `result` when it is one.
 * @param intervals - configured cadence.
 * @returns the delay in milliseconds, or undefined after a terminal state.
 */
export declare function nextPollDelay(result: ChangeRequestSnapshot | ForgeFailure, failures: number, intervals: PollIntervals): number | undefined;
//# sourceMappingURL=schedule.d.ts.map