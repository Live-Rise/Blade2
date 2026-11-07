/**
 * The page's own statement of why it is not showing the shared Session, and what the reader can do.
 *
 * The viewer renders the normal client UI, so a page that cannot read the share would otherwise leave
 * an empty shell with no way forward. Every state below therefore carries a plain statement, most of
 * them a hint, and the ones with something to do offer the action itself: re-read, sign in again, ask
 * the owner, or go back to the share directory. No state ends at an error code the reader cannot use.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-status-banner
 */
import { type ViewerTranslate } from './locales.ts';
import type { ViewerStatus } from './viewer-status.ts';
/** One thing the reader can do about the state the page is in. */
export type ViewerStatusAction = 'reload' | 'sign-in' | 'directory' | 'retry';
/** One page's statement, rebuilt in place so it never stacks duplicates. */
export interface ViewerStatusBanner {
    /**
     * State the one state the page is in, with the ways out that state offers.
     * @param status - State the page holds.
     */
    update(status: ViewerStatus): void;
    /** Remove the element and its listeners. */
    dispose(): void;
}
/**
 * Mount the page's statement of its own state.
 * @param doc - Document the page renders in.
 * @param onAction - Performs one action the reader asked for.
 * @param t - Translate seat bound to this package's namespace.
 * @returns The banner, which updates one element in place.
 */
export declare function mountViewerStatusBanner(doc: Document, onAction: (action: ViewerStatusAction) => void, t: ViewerTranslate): ViewerStatusBanner;
//# sourceMappingURL=viewer-status-banner.d.ts.map