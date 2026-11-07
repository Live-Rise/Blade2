import type { SessionRenameDialogProps } from '../contract/slots.ts';
/**
 * The `shell.overlay` entry: nothing while no rename is requested, otherwise
 * one dialog per request (keyed by the Session, so a new request starts a
 * fresh draft). Sessions have no client-side name-conflict rule (the host
 * normalizes), and unlike Workspace rename an unchanged title is NOT
 * blocked. An unnamed Session starts with an empty draft and requires a name.
 * @param props - the request hook, its settlement, the rename hop, and the locale seat.
 * @returns the open dialog, or null.
 */
export declare function SessionRenameDialog({ useRenameRequest, settleSessionRename, renameSession, t }: SessionRenameDialogProps): import("react").JSX.Element | null;
//# sourceMappingURL=RenameSession.d.ts.map