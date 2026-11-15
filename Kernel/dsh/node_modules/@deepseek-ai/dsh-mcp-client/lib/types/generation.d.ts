/** One transport lifetime, including pending work and confirmed closure. */
import type { Client } from '@modelcontextprotocol/client';
import type { Config } from './index.ts';
import type { McpCloseResult } from './types.ts';
/** Tracks actual tasks separately from their interruptible callers. */
export declare class McpGeneration {
    readonly client: Client;
    private readonly closeTimeoutMs;
    private readonly diagnostic;
    /** Cancellation shared by the transport and all work owned by this generation. */
    readonly lifetime: AbortController;
    private readonly pending;
    private readonly closed;
    private transport;
    private started;
    private closeObserved;
    private closing;
    constructor(client: Client, closeTimeoutMs: number, diagnostic: (error: unknown) => string);
    /**
     * Connect within the caller's remaining startup or probe budget.
     * @param config - resolved transport configuration.
     * @param timeoutMs - remaining operation budget; undefined leaves the SDK default.
     * @returns completion of negotiation, or cancellation/deadline rejection.
     */
    connect(config: Config, timeoutMs: number | undefined): Promise<void>;
    /** Reject when this generation was cancelled or its transport reported closure. */
    assertOpen(): void;
    /**
     * Track work through settlement, even when its caller times out or is cancelled.
     * @param operation - work receiving this generation's cancellation signal.
     * @param timeoutMs - optional caller deadline in milliseconds; does not cancel the work.
     * @returns the operation's result, or a cancellation/deadline rejection.
     */
    run<T>(operation: (signal: AbortSignal) => Promise<T>, timeoutMs?: number): Promise<T>;
    /**
     * Cancel callers, close the transport, and await owned work within the cleanup budget.
     * @returns the shared cleanup result, distinguishing transport closure from cleanup failure.
     */
    close(): Promise<McpCloseResult>;
    private finish;
}
//# sourceMappingURL=generation.d.ts.map