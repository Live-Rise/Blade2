/**
 * The one mode one share's owner explicitly grants this viewer page.
 *
 * Admission (anyone in the organization with the link, or an approved request) never implies a mode:
 * the page starts at `view`, an older share that never granted anything stays there, and an answer
 * the page cannot read — or one naming a mode this build does not know — leaves it there rather than
 * assuming more.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-capabilities
 */
/**
 * What one owner lets a viewer do with a share, as one of two tiers.
 *
 * The same two tiers the owner's own permission presets name. `view` reads the shared Session and
 * drives its loop inside the granted workspace without writing it; `write` also modifies the files
 * inside that workspace. Neither reconfigures the Session, and forking is a separate switch the
 * share reports on its own.
 */
export type ViewerSessionMode = 'view' | 'write';
/** Owner-granted abilities this page holds for the share it presents. */
export interface ViewerCapabilities {
    /** Mode the share is in; `view` until an answer says otherwise. */
    readonly mode: ViewerSessionMode;
    /** Stable code behind that answer, when the host named one. */
    readonly reason?: string;
    /**
     * Whether the page's last read of the share answered nothing it could read.
     *
     * The mode still holds the narrowest value, because nothing was granted, but the page has not
     * confirmed what the share is: a surface that states a reason says so instead of stating the
     * read-only fact as though the share had answered.
     */
    readonly unconfirmed?: true;
}
/** The one capability set the page holds; the mode starts at the narrowest one. */
export declare const viewerCapabilities: import("@deepseek-ai/dsh-client-store").SnapshotStore<ViewerCapabilities>;
/**
 * Return the capability store to the state a page that read nothing holds.
 *
 * The viewer session plugin owns the store's lifetime and calls this when its page starts and ends, so
 * a remount or HMR replacement cannot inherit the tier another page's share granted.
 */
export declare function resetViewerCapabilities(): void;
/**
 * Refusal codes that settle the page itself: the share is gone, expired, or this reader lost it.
 *
 * Nothing this page does can change that verdict, so every Session it holds states the reason instead
 * of the read-only fact it states while the share is merely read-only.
 */
export declare const PAGE_WIDE_REFUSALS: ReadonlySet<string>;
/** Every tier an owner may set, narrowest first. An answer naming anything else stays `view`. */
export declare const SESSION_MODES: readonly ViewerSessionMode[];
/**
 * Read one tier a Host reported.
 *
 * The withdrawn `fork-readonly` tier reads as `view`: it granted forking, which the share's own fork
 * switch carries now, so a link created under it keeps its width without naming a tier this build no
 * longer offers.
 * @param value - Value a Host reported, of any type.
 * @returns The tier, or undefined when this build does not know it.
 */
export declare function readSessionMode(value: unknown): ViewerSessionMode | undefined;
/**
 * Read what one `/__liveshare/context` answer grants this page.
 * @param context - Decoded context body, or undefined when unreadable.
 * @returns The mode this page holds after that answer.
 */
export declare function readViewerCapabilities(context: unknown): ViewerCapabilities;
/**
 * Publish what one context answer grants this page.
 * @param context - Decoded context body, or undefined when unreadable.
 */
export declare function publishViewerCapabilities(context: unknown): void;
//# sourceMappingURL=viewer-capabilities.d.ts.map