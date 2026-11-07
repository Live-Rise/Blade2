/**
 * The share dialog's plugin-endpoint tier control and the helpers that read a stored selection back
 * as one tier.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/ShareCapabilitiesPicker
 */
import type { ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { ViewerCapabilityGroup } from './share-types.ts';
/** One share-wide exposure tier the owner picks for every plugin that declares endpoints. */
export type CapabilityTier = 'none' | 'read' | 'write';
/**
 * The endpoints one tier exposes, expanded from what the plugins declare now.
 *
 * `read` carries every readable endpoint and no writable one; `write` carries both kinds, because it
 * is the wider tier rather than writable endpoints alone. A plugin with nothing to expose under the
 * tier is left out, exactly as a group that names no endpoint. Expansion at save time is what keeps a
 * record an explicit endpoint list: an endpoint a plugin declares later is not in this expansion, so
 * it stays closed until the owner saves again.
 * @param offered - Endpoints the host currently offers.
 * @param tier - Tier to expand.
 * @returns The `capabilities` array the owner API validates against the same offers.
 */
export declare function tierSelection(offered: readonly ViewerCapabilityGroup[], tier: CapabilityTier): readonly ViewerCapabilityGroup[];
/**
 * The endpoints one group list names, as comparable keys.
 *
 * The plugin and the kind are part of each key because the same name may be offered as readable by one
 * plugin and writable by another.
 * @param groups - Groups read from a share record.
 * @returns One key per named endpoint, sorted.
 */
export declare function selectionKeys(groups: readonly ViewerCapabilityGroup[]): readonly string[];
/**
 * Whether two key lists name exactly the same endpoints.
 * @param left - One sorted key list.
 * @param right - The other sorted key list.
 * @returns True when both name the same endpoints once each.
 */
export declare function sameKeys(left: readonly string[], right: readonly string[]): boolean;
/**
 * The tier one share's stored selection is, and whether it is a whole tier at all.
 *
 * A record that matches no tier is not shown as one: the control holds the default and the panel
 * states that saving replaces the record, because the record names a subset no tier represents — a
 * selection made before every endpoint was offered again, or one this page cannot attribute.
 * @param offered - Endpoints the host currently offers.
 * @param stored - Selection the host reported for this share.
 * @returns The matching tier, or the default with `stale` when the record matches none.
 */
export declare function readStoredTier(offered: readonly ViewerCapabilityGroup[], stored: readonly ViewerCapabilityGroup[]): {
    readonly tier: CapabilityTier;
    readonly stale: boolean;
};
/** Props for the endpoint tier control. */
export interface ShareCapabilitiesPickerProps extends PropsLocale<typeof NS> {
    /** Endpoints the mounted plugins currently offer; an empty list renders nothing. */
    readonly offered: readonly ViewerCapabilityGroup[];
    /** Tier the control acts on. */
    readonly tier: CapabilityTier;
    /** Whether the stored record matches no tier, so saving replaces it. */
    readonly stale: boolean;
    /** Whether the share's save is in flight. */
    readonly disabled: boolean;
    /** Base id the control's own tab and panel ids derive from. */
    readonly controlId: string;
    /** Record the owner's tier pick. */
    readonly onPick: (tier: CapabilityTier) => void;
}
/**
 * Render the endpoint tier control, its caveats, and what the current tier exposes.
 * @param props - The live offers, the tier in force, and localized copy.
 * @returns The control, or nothing when no plugin offers an endpoint.
 */
export declare function ShareCapabilitiesPicker({ offered, tier, stale, disabled, controlId, onPick, t, }: ShareCapabilitiesPickerProps): ReactNode;
//# sourceMappingURL=ShareCapabilitiesPicker.d.ts.map