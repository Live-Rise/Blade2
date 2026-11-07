/** Global owner-side list of shares; each row opens that Session's share dialog. */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
/** Props for the global Liveshare share manager. */
export interface LiveshareManageSharesDialogProps extends PropsLocale<typeof NS> {
    /** Owner share management, addressed through the generated `liveshareShares` namespace. */
    readonly shares: OwnerShares;
    /** Open the share dialog for one Session. */
    readonly onOpenShare?: (sessionId: string, sessionTitle: string) => void;
    /** Close the manager. */
    readonly onClose: () => void;
}
/**
 * List every share and open the one whose Session the owner wants to manage.
 * @param props - Owner shares, navigation callback, close callback, and copy.
 * @returns The manager modal.
 */
export declare function LiveshareManageSharesDialog({ shares, onOpenShare, onClose, t }: LiveshareManageSharesDialogProps): ReactNode;
//# sourceMappingURL=ManageSharesDialog.d.ts.map