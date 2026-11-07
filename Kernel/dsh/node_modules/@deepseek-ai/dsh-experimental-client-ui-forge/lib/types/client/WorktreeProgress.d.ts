/**
 * Card above the composer while a Session's worktree is created, while a message
 * waits after a failed or stopped attempt, and when its worktree is gone or
 * being restored.
 */
import { type ReactNode } from 'react';
import type { HostObservable, InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { PanelSnapshot } from './source.ts';
/** Business face of the card. */
export interface WorktreeProgressInjected {
    /** Panel snapshot of the Session; the renderer binds it as `usePanel`. */
    readonly hooks: {
        readonly panel: HostObservable<PanelSnapshot>;
    };
    /** Stop the running attempt a message waits for; the message keeps waiting. */
    readonly stop: () => void;
    /** Make another attempt at the worktree a message waits for. */
    readonly retry: () => void;
    /** Withdraw the worktree request, so the waiting message goes on without a worktree. */
    readonly withdraw: () => void;
    /**
     * Check out the removed worktree again.
     * @returns the failure text, or undefined once it is restored.
     */
    readonly restore: () => Promise<string | undefined>;
    /** Move the Session back to the source checkout of its removed worktree. */
    readonly leave: () => void;
}
/** Full props of the card. */
export type WorktreeProgressProps = InjectFace<WorktreeProgressInjected> & PropsLocale<typeof NS>;
/**
 * Render the worktree card of a Session: the running attempt at its
 * requested worktree, which can be stopped while a message waits for it; the
 * failed or stopped attempt a message waits after, with another attempt or
 * withdrawing the request so the message goes on without a worktree; or the
 * removal of the worktree the Session works in, with restoring it, whose
 * steps the card follows, and returning to the source checkout, after which
 * a waiting message goes on.
 * @param props - panel hook, actions, and copy.
 * @returns the card, or nothing while no attempt runs, no message waits, and the worktree exists.
 */
export declare function WorktreeProgress({ usePanel, stop, retry, withdraw, restore, leave, t }: WorktreeProgressProps): ReactNode;
//# sourceMappingURL=WorktreeProgress.d.ts.map