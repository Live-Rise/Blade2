/**
 * Delay schedule for one connection that keeps failing.
 *
 * The owner host's worker supervisor and the gateway's per-share Relay socket both reconnect rather
 * than give up, and both treat a connection that stayed up long enough as healthy, so the failure
 * after one starts the delays over instead of continuing the previous run's growth. Extracted from
 * the socket lifecycle so the schedule is deterministic and testable apart from spawning processes.
 */
/** Progress of one reconnect sequence. */
export interface RetryBackoff {
    /** Delay to wait before the next attempt. */
    readonly delayMs: number;
    /** Attempts since the last stable connection. */
    readonly attempt: number;
}
/** Schedule of a connection that has not (yet) proven itself stable. */
export declare const INITIAL_RETRY_BACKOFF: RetryBackoff;
/** Ceiling for the doubling delay: a failing peer is retried, never hammered. */
export declare const RETRY_BACKOFF_MAX_MS = 10000;
/**
 * A connection that stays up this long is healthy, so the next failure starts the delays over.
 * A peer that replaces the connection right after the handshake keeps the previous backoff instead
 * of retrying every 500 ms.
 */
export declare const RETRY_STABLE_MS = 30000;
/**
 * Advance the schedule after one failed attempt.
 * @param state Backoff of the attempt that just failed.
 * @returns Schedule the retry starts from: the delay to wait and the state after that attempt.
 */
export declare function retryBackoffNext(state: RetryBackoff): RetryBackoff;
/**
 * Schedule for a connection that has proven itself stable.
 * @returns The schedule's first entry, so the next failure is retried promptly.
 */
export declare function retryBackoffSettled(): RetryBackoff;
//# sourceMappingURL=retry-backoff.d.ts.map