/** Shared settlement deadline for the startup and probe budgets. */
/**
 * Settle with `promise`, or reject with `timeoutMessage` once `remainingMs`
 * elapses first. The timer never holds the process open, and a non-Error
 * rejection is wrapped so callers always observe an Error.
 * @param promise - operation the deadline bounds.
 * @param remainingMs - remaining budget in milliseconds (clamped to at least 1).
 * @param timeoutMessage - diagnostic carried by the deadline rejection.
 * @returns the operation's value; rejects with the deadline or the operation's failure.
 */
export declare function withDeadline<T>(promise: Promise<T>, remainingMs: number, timeoutMessage: string): Promise<T>;
//# sourceMappingURL=deadline.d.ts.map