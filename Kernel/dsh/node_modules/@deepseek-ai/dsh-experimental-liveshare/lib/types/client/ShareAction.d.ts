/** Session-header action that opens the Liveshare owner-side share dialog. */
import { type ReactNode } from 'react';
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
/** Business face the header action receives: the owner's share-management Remote. */
export interface LiveshareShareActionInjected {
    /** Owner share management, addressed through the generated `liveshareShares` namespace. */
    readonly shares: OwnerShares;
}
/** Full props for the Liveshare Session-header utility. */
export type LiveshareShareActionProps = PropsRuntime<'conversation.session.header.utilities'> & InjectFace<LiveshareShareActionInjected> & PropsLocale<typeof NS>;
/**
 * Render the Session Header Share action and its dialog for a local owner.
 * @param props - Session runtime plus localized copy.
 * @returns the header action and optional dialog.
 */
export declare function LiveshareShareAction({ sessionId, useSessions, shares, t }: LiveshareShareActionProps): ReactNode;
//# sourceMappingURL=ShareAction.d.ts.map