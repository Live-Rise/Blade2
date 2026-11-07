/** The glow a reveal draws once on the pane holding the revealed tab; the stylesheet owns its animation. */
import type { SidebarRightFlash } from '../service.ts';
/**
 * Mark the pane a flash request names, docked or floating, until its animation ends.
 * @param root - the Session's panel, which contains its docked and floating panes.
 * @param flash - the latest request; alternating serials select alternating keyframes, so a repeat restarts.
 * @returns the cleanup that removes the mark, or undefined when no pane was marked.
 */
export declare function flashPane(root: HTMLElement | null, flash: SidebarRightFlash | undefined): (() => void) | undefined;
//# sourceMappingURL=flash.d.ts.map