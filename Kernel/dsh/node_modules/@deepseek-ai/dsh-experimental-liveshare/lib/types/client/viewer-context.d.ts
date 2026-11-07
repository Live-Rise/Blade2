/**
 * The viewer page's own read of the share it presents.
 *
 * One answer carries both halves of the page's identity: which DSH the owner runs and who shared this
 * Session, and what the owner lets this reader do beyond reading. The page owns that read rather than
 * a component, because every surface re-reads what the share says while it renders.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-context
 */
/** Owner and build facts the Relay reports for the share this page presents. */
export interface ViewerContextFacts {
    readonly owner?: string;
    /** Verified subject of the share's owner; the label of last resort when no name was reported. */
    readonly ownerSubjectId?: string;
    /** Display name the Relay reported for the reader viewing this page. */
    readonly viewerDisplayName?: string;
    /** Subject the Relay reported for the reader viewing this page. */
    readonly viewerSubjectId?: string;
    readonly dshVersion?: string;
    readonly relay?: string;
}
/** The facts known so far; absent fields stay unstated. */
export declare const viewerContextFacts: import("@deepseek-ai/dsh-client-store").SnapshotStore<ViewerContextFacts>;
/**
 * Forget the previous page's read and facts for the page that takes over.
 *
 * The store and the counter are module-level so every surface reads one page's answer without a
 * service; the viewer session plugin owns their lifetime and calls this when its page starts and ends,
 * so a remount or HMR replacement never inherits another page's share. Advancing the generation also
 * retires any read the previous page left in flight, so its late answer cannot publish here.
 */
export declare function resetViewerContext(): void;
/**
 * Read the share and optional build facts under one generation.
 *
 * Only the newest uncancelled read may publish. A usable context confirms the share's own answer and
 * clears prior notes about it; context failures close abilities and keep the page unconfirmed. Build
 * failures affect neither the share state nor the context.
 * @param basePath - Public Relay base path the served document carries.
 * @param shareId - Share this page presents.
 * @param signal - Page lifetime cancellation; absent reads expire only under their deadline.
 * @returns Completion after both reads settle; superseded or cancelled answers publish nothing.
 */
export declare function readViewerContext(basePath: string, shareId: string, signal?: AbortSignal): Promise<void>;
//# sourceMappingURL=viewer-context.d.ts.map