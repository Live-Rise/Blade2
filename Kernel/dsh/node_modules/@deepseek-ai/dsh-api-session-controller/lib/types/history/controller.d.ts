/** Independent Session snapshots and passive event subscriptions. */
import type { Context } from '@deepseek-ai/cordis';
import type { SessionId } from '@deepseek-ai/dsh-session';
import type { SessionObservation } from '@deepseek-ai/dsh-session-query';
import { SessionAssistantStreamsListener } from './assistant-stream.ts';
import type { SessionEventFrame, SessionEventsPage, SessionPageRequest, SessionObserveEventRequest, SessionProjectionsRequest, SessionProjectionFrame, SessionAddress, SessionProjectionBaseline, SessionProjectionsValue, SessionWaitOutput } from '../types.ts';
/** Session snapshots do not share lifetime with passive subscriptions. */
export declare class HistoryController {
    private readonly ctx;
    /** Reconnect state for this controller's assistant subscriptions. */
    readonly streamsListener: SessionAssistantStreamsListener;
    /** @param ctx - Host context carrying Session reads and projections. */
    constructor(ctx: Context);
    /**
     * Read a passive page of Session events at an exact cut.
     * @param request - Session identity and optional cut and page window.
     * @param signal - read cancellation.
     * @returns a page at the supplied cut, or the observed log end when omitted.
     */
    events(request: SessionPageRequest, signal: AbortSignal): Promise<SessionEventsPage>;
    /**
     * Subscribe to Session events without reading or activating the Session.
     * @param request - Session address to subscribe.
     * @param signal - stream cancellation.
     * @returns subscription readiness followed by event frames; readiness does not establish existence.
     */
    observeEvent(request: SessionObserveEventRequest, signal: AbortSignal): AsyncIterable<SessionEventFrame>;
    /**
     * Read exact projections or migration cache hints without activating the Session.
     * @param request - Session identity whose exact projection baseline is required.
     * @param signal - read cancellation.
     * @returns exact sequenced values, migration cache hints, or null when absent.
     */
    projections(request: SessionProjectionsRequest, signal: AbortSignal): Promise<SessionProjectionsValue>;
    /**
     * Subscribe to projection changes without reading a baseline.
     * @param request - Session identity to subscribe without reading a baseline.
     * @param signal - stream cancellation.
     * @returns readiness followed by one change frame per committed event, listing every key it changed.
     */
    observeProjections(request: SessionProjectionsRequest, signal: AbortSignal): AsyncIterable<SessionProjectionFrame>;
    /**
     * Read bounded committed assistant messages through a stopping cursor.
     * @param sessionId - Session whose committed assistant messages are requested.
     * @param afterSeq - exclusive lower bound.
     * @param throughSeq - inclusive stopping-state cursor.
     * @param maxMessages - positive assistant record limit.
     * @param signal - caller cancellation, independent of a completed wait deadline.
     * @returns newest-first assistant messages and whether further matching records were omitted.
     */
    assistantOutput(sessionId: SessionId, afterSeq: number, throughSeq: number, maxMessages: number, signal: AbortSignal): Promise<SessionWaitOutput>;
    /**
     * Acquire a read without activating a Session; projections are opt-in.
     * @param request - Session identity and whether to compute projections.
     * @param signal - read cancellation.
     * @param callback - omitted to transfer lease ownership to the caller.
     * @returns a lease the caller must dispose.
     */
    usingObservation(request: {
        readonly sessionId: SessionId;
        readonly projection: boolean;
    }, signal: AbortSignal, callback?: undefined): Promise<SessionObservation>;
    /**
     * Keep the borrowed lease alive until the callback settles, then dispose it.
     * @param request - Session identity and whether to compute projections.
     * @param signal - read cancellation.
     * @param callback - synchronous or asynchronous computation; retain a separate lease if it must escape.
     * @returns the callback result; read and callback failures propagate.
     */
    usingObservation<T>(request: {
        readonly sessionId: SessionId;
        readonly projection: boolean;
    }, signal: AbortSignal, callback: (observation: SessionObservation) => T): Promise<Awaited<T>>;
}
/**
 * Validate an Address against the exact read used by a Controller.
 * @param address - caller-supplied Session address.
 * @param observation - borrowed read; subagent addresses require projections.
 */
export declare function validateObservationAddress(address: SessionAddress, observation: SessionObservation): void;
/**
 * Convert a borrowed observation to one page without acquiring or retaining it.
 * @param observation - exact read cut owned by the caller.
 * @param request - validated page window and optional inclusive cut.
 * @returns a chronological page at the requested cut.
 */
export declare function history(observation: SessionObservation, request: Omit<SessionPageRequest, 'address'> & {
    readonly maxMessages: number;
}): SessionEventsPage;
/**
 * Convert the projection baseline without acquiring or retaining its observation.
 * @param observation - projected read cut owned by the caller.
 * @returns the exact baseline; missing projection support fails explicitly.
 */
export declare function projections(observation: SessionObservation): SessionProjectionBaseline;
/**
 * Validate and resolve the history page window.
 * @param request - external history window.
 * @returns the validated request with an explicit message budget.
 */
export declare function resolveHistoryRequest(request: SessionPageRequest): SessionPageRequest & {
    readonly maxMessages: number;
};
//# sourceMappingURL=controller.d.ts.map