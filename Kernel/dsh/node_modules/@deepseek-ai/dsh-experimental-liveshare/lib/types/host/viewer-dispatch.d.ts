/**
 * Dispatch of one reader's viewer requests to the owner's own Connection and gateway.
 *
 * Endpoint classification, the source and child write paths, streams, page assets, and the request
 * entry all live here. Each handler re-reads the share's lifecycle and the policy's write refusal at
 * the points the original order required, and every resource it opens is tracked by viewer-lifecycle.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-dispatch
 */
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import { type ViewerAnswer, type ViewerRequest } from '../viewer-channel.ts';
import type { ViewerReadDeps } from './viewer-read.ts';
import type { ViewerLifecycle } from './viewer-lifecycle.ts';
import type { ViewerFork } from './viewer-fork.ts';
/**
 * The caller's stable identifier for one logical fork request.
 *
 * An explicit request field wins; otherwise the caller's `rpcId` names that same intent, because a
 * retry re-sends the same envelope. The boundary is deliberately not read here.
 * @param envelope - Opaque request body the viewer sent.
 * @returns The operation id, or undefined when the caller named no operation at all.
 */
export declare function requestedOperationId(envelope: unknown): string | undefined;
/**
 * The correlation id of one request envelope, when it carries one.
 * @param envelope - Opaque request body the viewer sent.
 * @returns The caller's id, or undefined.
 */
export declare function requestedRpcId(envelope: unknown): string | undefined;
/**
 * The completed-history prefix one fork request asks its child to branch from.
 *
 * A body that names none, or names something other than a safe integer, leaves the boundary unnamed:
 * the fork path then branches at the share's last completed prefix, or at 0 when the share published
 * none.
 * @param body - Opaque request body the viewer sent.
 * @returns The safe-integer boundary, or undefined when the request names none.
 */
export declare function requestedBoundary(body: unknown): number | undefined;
/**
 * The Session one prompt, cancel, or fork envelope names.
 * @param envelope - Opaque request body the viewer sent.
 * @returns The `sessionId` of the request record, or undefined when it names none.
 */
export declare function requestedSessionId(envelope: unknown): string | undefined;
/**
 * The ordinary Session a `session/page` or `session/follow` request addresses.
 * @param envelope - Unary envelope or stream payload the viewer sent.
 * @returns The Session id for a `session` address, or undefined for any other address.
 */
export declare function requestedAddressSession(envelope: unknown): string | undefined;
/** Host services one dispatched viewer request reads. */
export interface ViewerDispatchDeps {
    /** Host services the reader was built with. */
    readonly deps: ViewerReadDeps;
    /** Lifecycle state, refusal answers, and the resources a request opens. */
    readonly lifecycle: ViewerLifecycle;
    /** Fork orchestration, which a fork call and the branch annotations go through. */
    readonly fork: () => ViewerFork;
}
/** The one entry a viewer reader exposes for forwarded requests. */
export interface ViewerDispatch {
    /**
     * Answer one request for one share.
     * @param shareId - Share the Relay addressed.
     * @param request - Validated viewer request.
     * @param actor - Viewer identity the Relay asserted.
     * @param signal - Cancellation of the forwarded request.
     * @returns The answer the Relay writes back to the viewer.
     */
    answer(shareId: string, request: ViewerRequest, actor: ShareRpcActor, signal?: AbortSignal): Promise<ViewerAnswer>;
    /**
     * Dispatch one ordinary owner call for one admitted viewer.
     * @param path - Owner endpoint path.
     * @param body - Opaque request body.
     * @param shareId - Share the request arrived through.
     * @param actor - Viewer identity the Relay asserted.
     * @param signal - Cancellation of the forwarded request.
     * @returns The owner's own answer.
     */
    dispatchCall(path: string, body: unknown, shareId: string, actor: ShareRpcActor, signal?: AbortSignal): Promise<ViewerAnswer>;
}
/**
 * Build the request dispatch for one viewer reader.
 * @param input - Host services, lifecycle state, and the fork orchestrator.
 * @returns The reader's request entry and the plain owner-call dispatch a fork uses.
 */
export declare function createViewerDispatch(input: ViewerDispatchDeps): ViewerDispatch;
//# sourceMappingURL=viewer-dispatch.d.ts.map