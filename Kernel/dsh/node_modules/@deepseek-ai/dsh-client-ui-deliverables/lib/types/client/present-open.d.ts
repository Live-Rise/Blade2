import type { SessionId } from '@deepseek-ai/dsh-session/types';
import { type PresentedAction, type PresentedHost } from '../presented.ts';
/** Failure feedback for the shared native opening control. */
export type PresentedOpenFailure = 'openError' | 'revealError';
/**
 * Settled result of one native file gesture, for the contributed control to
 * announce: the Host acknowledged an application open or a file-manager
 * reveal, the gesture failed, or the request was deliberately dropped because
 * the same file was already opening or this controller was disposed. Only an
 * acknowledged operation may be announced as a success.
 */
export type PresentedOpenOutcome = {
    readonly kind: 'opened';
} | {
    readonly kind: 'revealed';
} | {
    readonly kind: 'failed';
    readonly failure: PresentedOpenFailure;
} | {
    readonly kind: 'skipped';
};
/** State published on the owning file card. */
export type PresentedOpenPhase = 'opening' | 'revealing' | 'error' | 'revealError' | 'nativeUnavailable';
/** One browser plugin's file-open requests, cancelled when that plugin is disposed. */
export declare class PresentedOpenController {
    /** File action URLs key the state across Sessions, turns, and both clickable surfaces. */
    readonly state: import("@deepseek-ai/dsh-client-store").SnapshotStore<Record<string, PresentedOpenPhase | undefined>>;
    /** Native destination metadata, or a retryable read failure. */
    readonly host: import("@deepseek-ai/dsh-client-store").SnapshotStore<PresentedHost | "error" | null>;
    private loading;
    private metadata;
    private readonly lifetime;
    private readonly pending;
    /**
     * Open a declared file once while a request for the same coordinates is pending.
     * Failures remain visible on the card and a later gesture retries them.
     * @param sessionId - viewed Session, including a fork's own identity.
     * @param seq - durable delivery event sequence.
     * @param index - original file index within that event.
     * @param action - default application open or file-manager reveal.
     * @param application - registered handler identifier for an explicit application choice.
     * @returns the settled outcome after publishing card status; a dropped
     * request reports `{ kind: 'skipped' }` and announces nothing.
     */
    open(sessionId: SessionId, seq: number, index: number, action?: PresentedAction, application?: string): Promise<PresentedOpenOutcome>;
    /**
     * Open one recorded changed file in the Host's default application.
     * @param sessionId - viewed Session.
     * @param seq - durable workspace/changes event sequence.
     * @param index - original file index within that event.
     * @param action - application open or file-manager reveal.
     * @param application - registered handler identifier for an explicit application choice.
     * @returns the settled outcome after publishing card status; a dropped
     * request reports `{ kind: 'skipped' }` and announces nothing.
     */
    openChanged(sessionId: SessionId, seq: number, index: number, action?: PresentedAction, application?: string): Promise<PresentedOpenOutcome>;
    private openUrl;
    /**
     * Read the serving desktop metadata, coalescing concurrent reads; a later call retries failure.
     * @returns after metadata or a retryable error is published.
     */
    loadHost(): Promise<void>;
    /** Invalidate desktop metadata on connection replacement; mounted cards request the new Host. */
    resetHost(): void;
    private readHost;
    /** Cancel outstanding requests, suppress their announcements, and wait until they settle. */
    dispose(): Promise<void>;
    private request;
}
//# sourceMappingURL=present-open.d.ts.map