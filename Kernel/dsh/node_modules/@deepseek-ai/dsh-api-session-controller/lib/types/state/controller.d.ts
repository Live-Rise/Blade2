/** Passive Session state, coalesced observations and quiescence waits. */
import type { Context } from '@deepseek-ai/cordis';
import type { HistoryController } from '../history/controller.ts';
import type { SessionState, SessionStateRequest, SessionWaitRequest, SessionWaitValue } from '../types.ts';
/** Reads and waits without activating or cancelling Session execution. */
export declare class StateController {
    private readonly ctx;
    private readonly historyController;
    /**
     * @param ctx - live Session and pending Remote event services.
     * @param historyController - passive read entry used for cold state and its bounded output.
     */
    constructor(ctx: Context, historyController: HistoryController);
    /**
     * Read passive execution and pending-input state.
     * @param request - Session identity.
     * @param signal - read cancellation.
     * @returns current passive state.
     */
    state(request: SessionStateRequest, signal: AbortSignal): Promise<SessionState>;
    /**
     * Observe complete passive state replacements.
     * @param request - Session identity.
     * @param signal - observation cancellation.
     * @returns complete state replacements, starting with the current state.
     */
    observeState(request: SessionStateRequest, signal: AbortSignal): AsyncIterable<SessionState>;
    /**
     * Wait for quiescence, pending input, or the deadline without cancelling execution.
     * @param request - Session identity, timeout and assistant output window.
     * @param signal - cancellation of this wait only, never of Agent execution.
     * @returns the stopping reason and last complete state; no state before timeout is an error.
     */
    wait(request: SessionWaitRequest, signal: AbortSignal): Promise<SessionWaitValue>;
    private listen;
    /**
     * @param sessionId - Session to sample.
     * @param signal - read cancellation.
     * @param retried - whether a live source already redirected this read once.
     * @returns one passive state sample.
     */
    private readState;
    private pendingInput;
}
//# sourceMappingURL=controller.d.ts.map