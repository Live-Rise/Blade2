/**
 * The share dialog's create form: the session permission, fork switch, and lifetime, plus the one
 * create action that publishes the Session through the host.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/ShareCreateForm
 */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { CreatedShare } from '../share-api-types.ts';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
import { type OwnerIdentityStatus } from './owner-identity.ts';
import type { RelayConnectivity, RelayPermission } from './share-relay-address.ts';
import { type CapabilityTier } from './ShareCapabilitiesPicker.tsx';
import type { ViewerCapabilityGroup } from './share-types.ts';
import { type ViewerSessionMode } from './viewer-capabilities.ts';
/** What the Host reported about the values the create request carried. */
export interface CreatedShareInfo {
    /** Session permission the Host stored, read through this build's vocabulary. */
    readonly reportedMode: ViewerSessionMode | undefined;
    /** Session permission the owner asked for. */
    readonly requestedMode: ViewerSessionMode;
    /** Whether the create request carried the fork switch. */
    readonly allowsFork: boolean;
}
/** Inputs the dialog supplies to its create form. */
export interface ShareCreateFormOptions {
    /** Owner share management the create action calls. */
    readonly shares: OwnerShares;
    /** Session the share exposes. */
    readonly sessionId: SessionId;
    /** Human-facing Session title sent with the share. */
    readonly sessionTitle: string;
    /** What the connectivity step established for the address in force. */
    readonly connectivity: RelayConnectivity;
    /** What the permission step established for this host at that address. */
    readonly permission: RelayPermission;
    /** The identity status the host reported for the address in force. */
    readonly ownerIdentity: OwnerIdentityStatus;
    /** Plugin endpoints the mounted plugins currently offer; none hides the endpoint control. */
    readonly offered: readonly ViewerCapabilityGroup[];
    /** Ask whether the address in force answers at all. */
    readonly testConnectivity: () => Promise<boolean>;
    /** Ask whether this host may publish a share at the address in force. */
    readonly testPermission: () => Promise<boolean>;
    /** State why the create action could not run, or clear the dialog's error with an empty line. */
    readonly onError: (message: string) => void;
    /** Adopt the record the host created, before its own listing confirms it. */
    readonly onCreated: (created: CreatedShare, info: CreatedShareInfo) => Promise<void>;
    /** The line the identity's own state states when it refuses creation. */
    readonly identityStateLine: () => string;
    /** Localized copy. */
    readonly t: PropsLocale<typeof NS>['t'];
}
/** Create-form state and the one create action the dialog's footer runs. */
export interface ShareCreateFormState {
    /** Whether the create request is in flight. */
    readonly busy: boolean;
    /** Session permission the share this dialog creates carries. */
    readonly sessionMode: ViewerSessionMode;
    /** The share's own fork switch. */
    readonly allowsFork: boolean;
    /** Plugin endpoint tier the share this dialog creates carries. */
    readonly capabilityTier: CapabilityTier;
    /** Plugin endpoints the mounted plugins currently offer. */
    readonly offered: readonly ViewerCapabilityGroup[];
    /** What the Host reported about the fork opt-in, stated when it cannot store it. */
    readonly modeOutcome: 'granted' | 'notGranted' | undefined;
    /** Base id the create form's session permission tabs derive from. */
    readonly sessionModeControlId: string;
    /** Base id the create form's plugin-endpoint tabs derive from. */
    readonly capabilityControlId: string;
    /** Pick the session permission the share this dialog creates carries. */
    readonly setSessionMode: (mode: ViewerSessionMode) => void;
    /** Set the share's own fork switch. */
    readonly setAllowsFork: (value: boolean) => void;
    /** Pick the plugin-endpoint tier the share this dialog creates carries. */
    readonly setCapabilityTier: (tier: CapabilityTier) => void;
    /** Publish the Session through the host and adopt the created record. */
    readonly create: () => Promise<void>;
    /** Whether a create request is running, readable from the close handler's closure. */
    readonly isCreating: () => boolean;
}
/**
 * Own the create form's values and the one create action.
 * @param options - Owner share management, the Session, the checks, and the adoption hooks.
 * @returns The form's values, ids, and create action.
 */
export declare function useShareCreateForm({ shares, sessionId, sessionTitle, connectivity, permission, ownerIdentity, offered, testConnectivity, testPermission, onError, onCreated, identityStateLine, t, }: ShareCreateFormOptions): ShareCreateFormState;
/** Props for the create form fields the dialog renders before the share exists. */
export interface ShareCreateFormProps extends PropsLocale<typeof NS> {
    /** The form's values and setters, as `useShareCreateForm` owns them. */
    readonly form: ShareCreateFormState;
    /** Whether a prerequisite (an owner sign-in) still makes every field unavailable. */
    readonly disabled?: boolean;
}
/**
 * Render the create form's session permission, fork switch, and lifetime fields.
 * @param props - The form state and localized copy.
 * @returns The fields shown while no share exists yet.
 */
export declare function ShareCreateForm({ form, disabled, t }: ShareCreateFormProps): ReactNode;
//# sourceMappingURL=ShareCreateForm.d.ts.map