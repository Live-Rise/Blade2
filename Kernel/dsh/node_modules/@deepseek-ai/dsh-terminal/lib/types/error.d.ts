/** Machine-routable PTY service and backend failures. @module */
import { HarnessError } from '@deepseek-ai/dsh-error';
/**
 * Machine-routable PTY service and backend failures. Backends use
 * `NO_SANDBOX` when a confined mode has no sandbox provider,
 * `SESSION_CLOSING` and `SESSION_EXITED` for an operation on a closing or
 * exited session, `STARTUP_EXITED` or `STARTUP_TIMEOUT` when the shell exits
 * or misses readiness during startup, and `CLEANUP_FAILED` (always as
 * {@link TerminalBackendCleanupError}) when startup failed and the partial
 * resources it left could not be cleaned. A consumer that drives a shell
 * through the service uses `INIT_REJECTED` when that shell rejects the
 * consumer's own setup input.
 */
export type TerminalErrorCode = 'CLEANUP_FAILED' | 'DUPLICATE_BACKEND' | 'DUPLICATE_NAME' | 'FOREIGN_SESSION' | 'INIT_REJECTED' | 'NO_BACKEND' | 'NO_SANDBOX' | 'NO_SESSION' | 'OWNER_NOT_LIVE' | 'SEND_ACTIVE' | 'SERVICE_DISPOSING' | 'SESSION_CLOSING' | 'SESSION_EXITED' | 'STARTUP_EXITED' | 'STARTUP_TIMEOUT';
/** Harness error carrying a stable {@link TerminalErrorCode}. */
export declare class TerminalError extends HarnessError {
    readonly code: TerminalErrorCode;
    /**
     * @param message - names the PTY operation and the safe next step.
     * @param code - the failure class consumers route on.
     * @param options - standard `cause` chaining.
     */
    constructor(message: string, code: TerminalErrorCode, options?: ErrorOptions);
}
/**
 * Backend-reported failure to clean partial resources after unpublished setup
 * failed; code `CLEANUP_FAILED`. Backend-owned resources may still be alive.
 * `errors` lists the setup failure, then the cleanup failure; `cause` is an
 * `AggregateError` of the same two failures, so renderers that follow `cause`
 * report both.
 */
export declare class TerminalBackendCleanupError extends TerminalError {
    readonly spawnError: unknown;
    readonly cleanupError: unknown;
    /** The setup failure followed by the cleanup failure. */
    readonly errors: readonly unknown[];
    /**
     * @param spawnError - original setup or cancellation failure.
     * @param cleanupError - failure that may leave backend-owned resources alive.
     */
    constructor(spawnError: unknown, cleanupError: unknown);
}
//# sourceMappingURL=error.d.ts.map