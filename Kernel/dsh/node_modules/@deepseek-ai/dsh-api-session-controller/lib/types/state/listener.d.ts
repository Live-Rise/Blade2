/** Coalesced Session state observations with Host-local revisions. */
import type { Context } from '@deepseek-ai/cordis';
import type { SessionId } from '@deepseek-ai/dsh-session';
import type { SessionState } from '../types.ts';
/** One complete state with its listener-local revision. */
export interface VersionedSessionState {
    state: SessionState;
    revision: number;
}
/** Coalesces Session/Agent invalidations with local revisions; pending events are sampled on reads. */
export declare class StateListener implements Disposable {
    private readonly readState;
    private readonly disposes;
    private readonly cancellation;
    private readonly signal;
    private revision;
    private resolver;
    private closed;
    /**
     * @param ctx - lifecycle notification owner.
     * @param target - Session to observe.
     * @param signal - caller cancellation.
     * @param readState - passive complete-state reader.
     */
    constructor(ctx: Context, target: SessionId, signal: AbortSignal, readState: (signal: AbortSignal) => Promise<SessionState>);
    /**
     * Check whether a completed read still represents the latest invalidation.
     * @param revision - revision captured with a completed read.
     * @returns whether no intervening invalidation occurred.
     */
    isCurrent(revision: number): boolean;
    /**
     * Read coalesced state replacements with optional periodic refresh.
     * @param refreshMs - optional idle recheck interval; omitted for event-only observation.
     * @returns complete state replacements with internal consistency revisions.
     */
    observeStates(refreshMs?: number): AsyncGenerator<VersionedSessionState>;
    [Symbol.dispose](): void;
}
//# sourceMappingURL=listener.d.ts.map