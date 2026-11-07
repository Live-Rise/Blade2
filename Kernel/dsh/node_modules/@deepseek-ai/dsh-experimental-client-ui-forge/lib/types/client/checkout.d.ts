/** What the panel's checkout row says about the checkout the Session works in. */
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { ForgePanelState } from '../types.ts';
import type { NS } from './locales.ts';
type Translate = PropsLocale<typeof NS>['t'];
/** Facts the checkout row shows. */
export interface CheckoutLabel {
    /** Branch name, the abbreviated commit while HEAD is detached, or null when HEAD has no commit. */
    readonly name: string | null;
    /** Whether HEAD is detached. */
    readonly detached: boolean;
    /** Kind of directory: a worktree with its base, another worktree, the project directory, or a removed worktree. */
    readonly kind: string;
    /** The kind with the directory path, home abbreviated to `~`. */
    readonly tooltip: string;
    /** Whether the worktree the Session works in is gone. */
    readonly missing: boolean;
    /** Full path of the directory, home not abbreviated; a removed worktree keeps the path it was created at. */
    readonly path: string;
    /** Whether the directory is a worktree rather than the project directory. */
    readonly worktree: boolean;
}
/**
 * Describe the checkout of a Session. A worktree this panel created names its
 * base; any other linked worktree is a worktree, and the main checkout is the
 * project directory. A removed worktree keeps its branch name.
 * @param state - panel state.
 * @param home - Host account home, or undefined to show full paths.
 * @param t - forge panel copy.
 * @returns the label, or undefined before a successful inspection.
 */
export declare function checkoutLabel(state: ForgePanelState, home: string | undefined, t: Translate): CheckoutLabel | undefined;
export {};
//# sourceMappingURL=checkout.d.ts.map