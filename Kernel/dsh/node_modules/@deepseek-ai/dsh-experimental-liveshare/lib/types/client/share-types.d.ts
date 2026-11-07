/** Shared owner-facing share records read from the owner share-management Remote. */
import type { ViewerSessionMode } from './viewer-capabilities.ts';
/**
 * One plugin's endpoints the owner may expose to a share's viewers.
 *
 * The same shape carries the plugins' live offers and one share's own selection: the panel presents
 * the offers as tick boxes and saves back the selected subset, so both sides read the same fields.
 */
export interface ViewerCapabilityGroup {
    /** Declaring plugin's name, which is what the owner ticks under. */
    readonly plugin: string;
    /** Readable endpoints of that plugin, sorted by the host. */
    readonly readable: readonly string[];
    /** Writable endpoints of that plugin, sorted by the host. */
    readonly writable: readonly string[];
}
/** One owner-created share listed by the global management UI. */
export interface OutgoingShare {
    readonly shareId: string;
    readonly sessionId: string;
    readonly sessionTitle: string;
    readonly link: string;
    readonly createdAt: string;
    /**
     * The session permission the owner set for this share. Absent when the owner's Harness does not
     * report a mode at all, which is stated as unsupported rather than as the narrowest one.
     */
    readonly sessionMode?: ViewerSessionMode;
    /**
     * Whether this share lets an admitted viewer fork it.
     *
     * Independent of the tier: absent on a Host that does not report it, which reads as off.
     */
    readonly allowsFork?: boolean;
    /**
     * Plugin endpoints this share exposes to its viewers, as the owner's Harness reports them.
     *
     * Absent when that Harness reports none, which is also what an empty list means: no plugin endpoint
     * reaches this share's viewers. A host that cannot report the field is read the same way, because
     * nothing was selected for the share.
     */
    readonly viewerCapabilities?: readonly ViewerCapabilityGroup[];
    /** ISO time after which the Relay denies viewers; absent means it never expires. */
    readonly expiresAt?: string;
    readonly online: boolean;
    readonly access?: unknown;
}
/** One durable audit entry shown in the owner share history. */
export interface ShareAuditEntry {
    readonly id: string;
    readonly action: string;
    readonly createdAt: string;
    readonly actor?: string;
}
/** Browser-safe summary of the persisted ACL scope for one share. */
export interface ConfirmedShareAccess {
    readonly mode: 'open' | 'selected';
    readonly selectedCount: number;
    readonly deploymentCount: number;
}
/**
 * Normalize one share access payload into a browser-safe summary.
 * @param value Raw `access` field from the owner share list.
 * @returns Confirmed mode and counts, or undefined for legacy or malformed records.
 */
export declare function readAccess(value: unknown): ConfirmedShareAccess | undefined;
/**
 * Normalize one host-reported endpoint list into the entries this page can present.
 *
 * A Host that reports nothing, reports a value this build cannot read, or reports one malformed entry
 * leaves this page with no offer to present for it: an unreadable offer is not one the owner may tick.
 * @param value Raw `capabilities` or `viewerCapabilities` field.
 * @returns The readable groups, in the order the host reported them.
 */
export declare function readCapabilityGroups(value: unknown): readonly ViewerCapabilityGroup[];
//# sourceMappingURL=share-types.d.ts.map