/**
 * Browser controller for the Session Header's "Copy Session as Markdown" action. One
 * plugin instance owns one pending copy: the clipboard is a browser-wide
 * destination, so a second request joins the pending operation instead of
 * racing it.
 *
 * On hosts with `ClipboardItem` and `navigator.clipboard.write`, the controller
 * starts that write synchronously from the click with a promised text Blob, so
 * the browser keeps the user activation while the Host prepares the
 * transcript. Other hosts fall back to `writeClipboard` after preparation. A
 * disposal aborts the read and prevents later text from reaching the Blob, but
 * an accepted operating-system clipboard write cannot be rolled back.
 * @module
 */
import { type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
/** Whether the latest copy attempt succeeded, prepared no transcript, or could not write. */
export type SessionMarkdownCopyNoticeKind = 'success' | 'preparationFailed' | 'clipboardFailed';
/** One settled copy notice; `seq` keys the toast so a repeat copy restarts it. */
export interface SessionMarkdownCopyNotice {
    readonly kind: SessionMarkdownCopyNoticeKind;
    readonly seq: number;
}
/** Copy state shared by every Header row and the root overlay toast. */
export interface SessionMarkdownCopyState {
    /** Whether this plugin instance has a copy operation still pending. */
    copying: boolean;
    /** Latest settled copy notice, or null while none shows. */
    notice: SessionMarkdownCopyNotice | null;
}
/** Browser clipboard operations used by one copy attempt. */
export interface SessionMarkdownClipboard {
    /** Whether a deferred write can start inside the click's activation window. */
    supportsDeferredWrite(): boolean;
    /** Start one deferred text write; the browser observes the data promise as it settles. */
    writeDeferred(data: Promise<Blob>): Promise<void>;
    /** Write prepared text on hosts without the deferred API; false means the browser rejected it. */
    writeText(text: string): Promise<boolean>;
}
/** HTTP carrier for the Markdown route. */
type Fetch = (input: string | URL, init?: RequestInit) => Promise<Response>;
/** Owns the one pending Markdown copy for this plugin instance. */
export declare class SessionMarkdownCopyController {
    private readonly fetcher;
    private readonly clipboard;
    /** uSES-safe state source shared by every Header row and the overlay toast. */
    readonly store: SnapshotStore<SessionMarkdownCopyState>;
    private pending;
    private disposed;
    private noticeSeq;
    constructor(fetcher?: Fetch, clipboard?: SessionMarkdownClipboard);
    /**
     * Copy one Session's complete transcript. A request received while a copy is
     * pending joins that operation and keeps its originally captured Session.
     * @param sessionId - Session captured by the click.
     * @returns after the clipboard work settles, a failure notice is published, or a late post-disposal request is ignored.
     */
    copy(sessionId: SessionId): Promise<void>;
    /** Clear the shown toast without affecting a pending copy. */
    dismiss(): void;
    /**
     * Abort a pending read, suppress a late clipboard write or notice, and reach
     * quiescence. A write already accepted by the operating system cannot be
     * undone.
     * @returns after the pending operation and any admitted clipboard work settle.
     */
    dispose(): Promise<void>;
    private run;
    private prepare;
    private settleDeferred;
    private settleFallback;
    private isCancelled;
    private publishNotice;
}
export {};
//# sourceMappingURL=copy-controller.d.ts.map