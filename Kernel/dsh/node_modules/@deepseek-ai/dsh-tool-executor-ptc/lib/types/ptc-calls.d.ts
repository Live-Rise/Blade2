/** Cancellation and settlement of complete PTC calls, including nested tools. */
import type { Context } from '@deepseek-ai/cordis';
import type { SessionId } from '@deepseek-ai/dsh-session';
import type { SandboxMode } from '@deepseek-ai/dsh-sandbox';
/** Serializes recovery with complete calls, including their nested-tool cleanup. */
export declare class PtcCalls {
    private readonly calls;
    private readonly resets;
    private disposed;
    /**
     * Observe policy changes for the owning executor.
     * @param ctx - Executor lifecycle and Session events.
     */
    constructor(ctx: Context);
    /**
     * Await canceled predecessors before resolving authority and admitting a call.
     * @param sessionId - Calling Session.
     * @param signal - Caller cancellation.
     * @param mode - Current standing mode, if supported, read after preceding cleanup.
     * @param run - Complete call, including nested-tool cleanup.
     * @returns The call result.
     */
    run<T>(sessionId: SessionId, signal: AbortSignal, mode: () => SandboxMode | undefined, run: (signal: AbortSignal) => Promise<T>): Promise<T>;
    /**
     * Cancel calls and release their environment independently of program cooperation.
     * A failed release blocks new calls until a later reset succeeds.
     * @param sessionId - Calling Session.
     * @param release - Host operation that terminates and cleans the owned environment.
     * @param notify - Queue the state-loss notice after cleanup and before admitting new calls.
     * @returns Completion after release and all canceled calls settle; concurrent resets share it.
     */
    reset(sessionId: SessionId, release: () => Promise<void>, notify?: () => void): Promise<void>;
}
//# sourceMappingURL=ptc-calls.d.ts.map