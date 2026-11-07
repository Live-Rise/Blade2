/**
 * Lifecycle state of one viewer reader: streams, calls, reader Peers, and share refusals.
 *
 * Every viewer request is admitted against the share's lifecycle here, and every resource a request
 * opens — a reader Peer, an open stream, an in-flight call, a child's enforcement — is released from
 * this module. Closing a share or a viewer walks exactly the resources that belong to it.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-lifecycle
 */
import type { PeerScope } from '@deepseek-ai/dsh-typert-protocol';
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import { type ForkShareState } from './fork-policy.ts';
import type { ReaderWriteAction } from './viewer-access-policy.ts';
import type { ViewerAnswer } from '../viewer-channel.ts';
import type { ViewerReadDeps } from './viewer-read.ts';
/**
 * How long one open viewer stream may stay unpolled before this host releases it.
 *
 * A page polls each open stream without pause and one poll's hold is {@link STREAM_POLL_HOLD_MS}, so a
 * stream nothing has polled for this long belongs to a page that is gone: closed, discarded by the
 * browser, or offline past its own reconnect backoff. Releasing it stops this host holding the stream
 * and the upstream subscription the owner's gateway opened for it. The value is many poll holds, so an
 * ordinary reconnect or one slow answer never costs a stream the page is still reading.
 */
export declare const VIEWER_STREAM_IDLE_TTL_MS = 120000;
/**
 * The refusal one share's own lifecycle applies, whatever a viewer asks through it.
 *
 * A share the owner stopped and one whose deadline passed authorize nothing. The write path reads these
 * two verdicts before it drives a Session; every read path reads them here, so a viewer cannot keep
 * reading a share the owner ended. Share state this host cannot read — a host that publishes none — is
 * not a refusal this function may name.
 * @param share - Share state as the owner host holds it, when it holds one.
 * @returns The refusal, or undefined while the share may serve.
 */
export declare function shareLifecycleRefusal(share: ForkShareState | undefined): ViewerAnswer | undefined;
/**
 * One stream this reader holds open for a viewer page, from the reservation that opened it until the
 * page stops polling it or the share closes.
 *
 * The page's polls drive `pending`, and an unpolled stream past `lastPolledAt`'s idle deadline is a page
 * that went away, so the reader releases the stream and the owner gateway's upstream subscription.
 */
export interface OpenStream {
    readonly shareId: string;
    /** Stream id the page numbered this read under, which is also its key within the share. */
    readonly id: number;
    readonly endpoint: string;
    /** Identity the stream opened as, re-checked on every poll. */
    readonly actor: ShareRpcActor;
    readonly controller: AbortController;
    iterator: AsyncIterator<unknown>;
    pending: Promise<IteratorResult<unknown>> | undefined;
    /** When this stream last answered a poll; an unpolled stream is a page that went away. */
    lastPolledAt: number;
}
/** The Peer one reader of one share speaks for. */
interface ReaderPeer {
    readonly shareId: string;
    readonly actor: ShareRpcActor;
    readonly peer: PeerScope;
}
/** One viewer call this reader is serving, tracked so a grant that ends can abort it. */
export interface ViewerCall {
    readonly shareId: string;
    readonly actor: ShareRpcActor;
    readonly controller: AbortController;
    readonly signal: AbortSignal;
    refreshing: boolean;
}
/** A stream opening reserved before the owner gateway answers. */
export interface OpeningReservation {
    readonly shareId: string;
    readonly actor: ShareRpcActor;
    readonly controller: AbortController;
}
/** Lifecycle state and refusal answers one viewer reader exposes. */
export interface ViewerLifecycle {
    /**
     * The refusal one share's own lifecycle applies, whatever a viewer asks through it.
     * @param shareId - Share the request arrived through.
     * @param actor - Viewer identity the Relay asserted.
     * @returns The refusal, or undefined while the share may still serve this viewer.
     */
    shareRefusal(shareId: string, actor: ShareRpcActor): ViewerAnswer | undefined;
    /**
     * The policy's refusal of one reader prompt or cancel on one Session, as the page's refusal answer.
     * @param shareId - Share the request arrived through.
     * @param actor - Viewer identity the Relay asserted.
     * @param action - `session/prompt` or `session/cancel`.
     * @param sessionId - Session the request names.
     * @param concrete - True for the call about to be dispatched, false for the snapshot rule.
     * @returns The refusal, or undefined while the policy allows the request.
     */
    accessRefusal(shareId: string, actor: ShareRpcActor, action: ReaderWriteAction, sessionId: string, concrete: boolean): (ViewerAnswer & {
        readonly reason: string;
    }) | undefined;
    /**
     * The share's lifecycle refusal followed by the policy's write refusal.
     * @param shareId - Share the request arrived through.
     * @param actor - Viewer identity the Relay asserted.
     * @param action - `session/prompt` or `session/cancel`.
     * @param source - Session the request names.
     * @returns The refusal, or undefined while the request may be dispatched.
     */
    trustedState(shareId: string, actor: ShareRpcActor, action: ReaderWriteAction, source: string): ViewerAnswer | undefined;
    /**
     * Refresh authoritative admission, marking the call whose signal it is as refreshing.
     * @param shareId - Share to refresh.
     * @param signal - Signal of the request being refreshed.
     */
    refreshAdmission(shareId: string, signal?: AbortSignal): Promise<void>;
    /**
     * Track one call as in flight.
     * @param shareId - Share the call belongs to.
     * @param actor - Caller identity.
     * @param controller - Controller that aborts this call.
     * @param signal - Signal that identifies this call during a refresh.
     * @returns The tracked call.
     */
    beginCall(shareId: string, actor: ShareRpcActor, controller: AbortController, signal: AbortSignal): ViewerCall;
    /**
     * Stop tracking one call.
     * @param call - The call that ended.
     */
    endCall(call: ViewerCall): void;
    /**
     * The Peer one reader of one share speaks for, created on its first dispatch.
     * @param shareId - Share the call belongs to.
     * @param actor - Reader identity.
     * @returns The Peer, or undefined when this host has no Connection.
     */
    peerFor(shareId: string, actor: ShareRpcActor): PeerScope | undefined;
    /**
     * Dispose every tracked Reader Peer that matches.
     * @param matches - Which Peers to dispose.
     */
    disposePeers(matches: (entry: ReaderPeer) => boolean): void;
    /**
     * One open stream.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     * @returns The stream, or undefined when none is open.
     */
    streamOf(shareId: string, id: number): OpenStream | undefined;
    /**
     * One stream opening reserved before the owner gateway answers.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     * @returns The reservation, or undefined when none is held.
     */
    openingOf(shareId: string, id: number): OpeningReservation | undefined;
    /**
     * Reserve one stream id before the owner gateway answers.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     * @param reservation - Reservation to hold.
     */
    reserveOpening(shareId: string, id: number, reservation: OpeningReservation): void;
    /**
     * Drop one opening reservation when it is still the one held.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     * @param reservation - Reservation the opener holds.
     * @returns True when this reservation was the one still held.
     */
    releaseOpening(shareId: string, id: number, reservation: OpeningReservation): boolean;
    /**
     * Record one opened stream and arm the idle sweep.
     * @param stream - Stream the owner gateway opened.
     */
    registerStream(stream: OpenStream): void;
    /**
     * Release one stream and the upstream subscription it holds.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     */
    release(shareId: string, id: number): void;
    /**
     * Whether one stream is still the open one under its key.
     * @param shareId - Share the stream belongs to.
     * @param id - Stream id the page numbered.
     * @param stream - Stream to compare.
     * @returns True while that stream is current and not aborted.
     */
    streamIsCurrent(shareId: string, id: number, stream: OpenStream): boolean;
    /** Arm the timer that reclaims streams nothing polls. */
    armSweep(): void;
    /**
     * Keep one child's enforcement release until its share or reader ends.
     * @param childSessionId - Child whose limit was installed.
     * @param releaseEnforcement - Disposer that releases the limit.
     */
    holdEnforcement(childSessionId: string, releaseEnforcement: () => void): void;
    /**
     * Cancel pending calls without releasing retained streams or child enforcement.
     * @param shareId - Share whose mode was narrowed.
     */
    cancelPendingCalls(shareId: string): void;
    /**
     * End only calls, streams, and reader Peers whose reader lost admission.
     * @param shareId - Share whose readers are re-checked.
     */
    recheckAdmission(shareId: string): void;
    /**
     * Cancel pending calls, release every stream, and dispose every reader Peer of one share.
     * @param shareId - Share whose pending calls, streams, and Peers end.
     */
    closeShare(shareId: string): void;
    /**
     * End only calls, streams, and reader Peers of one reader of one share.
     * @param shareId - Share the viewer's streams belong to.
     * @param viewer - Identity the Relay asserted when those streams opened.
     * @returns How many streams were released.
     */
    closeViewerStreams(shareId: string, viewer: ShareRpcActor): number;
    /** Cancel pending calls, release every open stream, and dispose every reader Peer. */
    close(): void;
}
/**
 * Build the lifecycle state of one viewer reader.
 * @param deps - Host services and the share-to-Session lookup.
 * @returns Lifecycle state and refusal answers.
 */
export declare function createViewerLifecycle(deps: ViewerReadDeps): ViewerLifecycle;
export {};
//# sourceMappingURL=viewer-lifecycle.d.ts.map