/** Liveshare owner-side share dialog. */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
/** Props for one session-scoped share dialog. */
export interface LiveshareShareDialogProps extends PropsLocale<typeof NS> {
    /** Session this share exposes. */
    readonly sessionId: SessionId;
    /** Human-facing Session title sent with the share; the panel deliberately does not print it. */
    readonly sessionTitle: string;
    /** Address the mounted share host reported as saved; empty while none is saved. */
    readonly defaultRelay: string;
    /** Owner share management, addressed through the generated `liveshareShares` namespace. */
    readonly shares: OwnerShares;
    /** Open the global share manager. */
    readonly onManage?: () => void;
    /** Close the dialog. */
    readonly onClose: () => void;
}
/**
 * Create one read-only Liveshare share and show its link.
 * @param props - Session identity, relay default, close callback, and copy.
 * @returns the share modal.
 */
export declare function LiveshareShareDialog({ sessionId, sessionTitle, defaultRelay, shares, onManage, onClose, t, }: LiveshareShareDialogProps): ReactNode;
//# sourceMappingURL=ShareDialog.d.ts.map