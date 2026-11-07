/**
 * The stored share's editable permission, fork switch, and plugin-endpoint exposure, with the one
 * save that submits all three through the host.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/ShareEditPanel
 */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { CreatedShare } from '../share-api-types.ts';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
import { type CapabilityTier } from './ShareCapabilitiesPicker.tsx';
import { readAccess, type ViewerCapabilityGroup } from './share-types.ts';
import { type ViewerSessionMode } from './viewer-capabilities.ts';
/** Inputs the dialog supplies to its stored-share edit state. */
export interface UseShareEditOptions {
    /** Owner share management the edit state reads and saves through. */
    readonly shares: OwnerShares;
    /** Session whose share this dialog edits. */
    readonly sessionId: SessionId;
    /** Adopt a link the host listed when the dialog has none yet. */
    readonly onLink: (link: string) => void;
    /** Localized copy. */
    readonly t: PropsLocale<typeof NS>['t'];
}
/** Stored-share values, the owner's unsaved picks, and the one save they share. */
export interface ShareEditState {
    /** Confirmed ACL scope of the share on hand, as the host reported it. */
    readonly access: ReturnType<typeof readAccess>;
    /** Plugin endpoints the mounted plugins currently offer; none hides the endpoint control. */
    readonly offered: readonly ViewerCapabilityGroup[];
    /** The tier the control and the save act on: the owner's own pick, or the record's whole tier. */
    readonly tier: CapabilityTier;
    /** Whether the record matches no tier, so saving replaces it instead of showing a tier it is not. */
    readonly stale: boolean;
    /** The session tier the owner edits here: their pick, or the one the host reported. */
    readonly shareMode: ViewerSessionMode | undefined;
    /** The fork switch the owner edits here: their pick, or the one the host reported (off when absent). */
    readonly shareFork: boolean;
    /** The share whose stored record this dialog edits; nothing until the host lists one. */
    readonly editableShare: string | undefined;
    /** Stored deadline; absent means the share never expires. */
    readonly expiresAt: string | undefined;
    /** Whether the share holds anything the owner has picked but not saved. */
    readonly dirty: boolean;
    /** Whether a close attempt already warned about unsaved changes; a second one closes. */
    readonly discardWarned: boolean;
    /** Whether the save is in flight. */
    readonly saveBusy: boolean;
    /** What the last accepted save stored, already localized. */
    readonly saveNotice: string;
    /** Why the last save did not land, already localized. */
    readonly saveError: string;
    /** Base id the tier control's own tab and panel ids derive from. */
    readonly capabilityControlId: string;
    /** Base id the share's own session permission tabs derive from. */
    readonly shareModeControlId: string;
    /** Record the owner's tier pick. */
    readonly pickTier: (tier: CapabilityTier) => void;
    /** Record the owner's session permission pick. */
    readonly pickMode: (mode: ViewerSessionMode) => void;
    /** Record the owner's fork-switch pick. */
    readonly pickFork: (fork: boolean) => void;
    /** Ask the save to clear the stored deadline and make the share never expire. */
    readonly clearExpiry: () => void;
    /** Save the session permission, the fork switch, and endpoint exposure through the host. */
    readonly savePermissions: () => Promise<void>;
    /** Record that a close attempt warned about unsaved changes. */
    readonly warnDiscard: () => void;
    /**
     * Adopt the record the host answered a create with.
     * @param created - The share the host created.
     * @param reportedMode - Session permission it stored, read through this build's vocabulary.
     * @param allowsFork - Whether the create request carried the fork switch.
     */
    readonly adoptCreated: (created: CreatedShare, reportedMode: ViewerSessionMode | undefined, allowsFork: boolean) => Promise<void>;
}
/**
 * Read the stored share record, keep the owner's unsaved picks, and save all three editable fields.
 * @param options - Owner share management, the Session, the link adoption callback, and localized copy.
 * @returns The stored values, the owner's picks, and the one save.
 */
export declare function useShareEdit({ shares, sessionId, onLink, t, }: UseShareEditOptions): ShareEditState;
/** Props for the stored share's permission section. */
export interface ShareEditPanelProps extends PropsLocale<typeof NS> {
    /** Whether a share exists to edit; the section renders nothing before one does. */
    readonly visible: boolean;
    /** Stored values, owner picks, and the one save they share. */
    readonly edit: ShareEditState;
}
/**
 * Render the stored share's session permission, fork switch, and endpoint tier.
 * @param props - The edit state, whether a share exists, and localized copy.
 * @returns The permission section, or nothing while no share exists.
 */
export declare function ShareEditPanel({ visible, edit, t }: ShareEditPanelProps): ReactNode;
//# sourceMappingURL=ShareEditPanel.d.ts.map