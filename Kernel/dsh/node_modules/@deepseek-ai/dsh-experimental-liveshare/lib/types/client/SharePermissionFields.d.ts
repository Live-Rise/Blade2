/**
 * The session permission, fork switch, and plugin-endpoint tier shared by the create form and the
 * stored-share editor.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/SharePermissionFields
 */
import type { ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import { type CapabilityTier } from './ShareCapabilitiesPicker.tsx';
import type { ViewerCapabilityGroup } from './share-types.ts';
import type { ViewerSessionMode } from './viewer-capabilities.ts';
/** Values and callbacks one shared permission field renders. */
export interface SharePermissionFieldsProps extends PropsLocale<typeof NS> {
    /** Session permission on hand; absent when the Host cannot place the stored tier. */
    readonly sessionMode: ViewerSessionMode | undefined;
    /** Whether no supported session permission exists, stated instead of an empty control. */
    readonly modeUnsupported?: boolean;
    /** The share's own fork switch. */
    readonly allowsFork: boolean;
    /** Plugin endpoints the mounted plugins offer. */
    readonly offered: readonly ViewerCapabilityGroup[];
    /** Endpoint tier on hand. */
    readonly tier: CapabilityTier;
    /** Whether the stored endpoints match no tier, so the picker states an unmatched selection. */
    readonly stale?: boolean;
    /** Whether every control is unavailable. */
    readonly disabled: boolean;
    /** Base id the session permission tabs derive from. */
    readonly sessionModeControlId: string;
    /** Base id the endpoint tabs derive from. */
    readonly capabilityControlId: string;
    /** Record a session permission pick. */
    readonly onSessionMode: (mode: ViewerSessionMode) => void;
    /** Record a fork-switch pick. */
    readonly onAllowsFork: (value: boolean) => void;
    /** Record an endpoint tier pick. */
    readonly onTier: (tier: CapabilityTier) => void;
}
/**
 * Render the fields both creating and editing a share present.
 * @param props - Values, callbacks, control ids, and localized copy.
 * @returns The shared permission rows.
 */
export declare function SharePermissionFields({ sessionMode, modeUnsupported, allowsFork, offered, tier, stale, disabled, sessionModeControlId, capabilityControlId, onSessionMode, onAllowsFork, onTier, t, }: SharePermissionFieldsProps): ReactNode;
//# sourceMappingURL=SharePermissionFields.d.ts.map