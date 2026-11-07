/**
 * Owner state file: the Relay override, the shares this host restores, and the Relay rows it owes.
 *
 * One JSON document holds everything this host must survive a restart with, and every write is
 * serialized per file so the newest content is the one that lands last.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/owner-persistence
 */
import type { ViewerSessionMode } from './fork-policy.ts';
import type { OwnerShare, ShareAccessSummary } from './owner-types.ts';
import type { ViewerCapabilityGroup } from './viewer-declarations.ts';
/** One share as the owner state file stores it. */
export interface PersistedOwnerShare {
    readonly shareId: string;
    readonly sessionId: string;
    readonly sessionTitle: string;
    /** Mode this share was created with; the retired `approval` spelling reads back as `selected`. */
    readonly accessMode: 'open' | 'selected' | 'approval';
    /** Relay this share was published to; a record without one cannot be restored. */
    readonly relay: string;
    readonly gatewayToken: string;
    readonly allowedSubjectIds: readonly string[];
    readonly access?: ShareAccessSummary;
    readonly createdAt: string;
    readonly expiresAt?: string;
    /**
     * Whether this share lets an admitted viewer fork it.
     *
     * Absent on every share written before the switch was stored on its own, and such a record reads as
     * off at the boundary that loads it.
     */
    readonly allowsFork?: boolean;
    /**
     * Tier this share was created with.
     *
     * Absent in files written before tiers existed, and such a record loads as `view`.
     */
    readonly sessionMode?: ViewerSessionMode;
    /**
     * Plugin endpoints this share exposed to its viewers when it was written.
     *
     * Absent on every share written before the capability existed, and absent means none: an upgrade never
     * exposes a plugin endpoint the owner had no chance to select.
     */
    readonly viewerCapabilities?: readonly ViewerCapabilityGroup[];
}
/** Owner state read from disk: the persisted Relay override, every restorable share, and every Relay row still owed. */
export interface LoadedOwnerState {
    readonly relayUrl?: string;
    readonly instanceId: string;
    readonly shares: Map<string, OwnerShare>;
    readonly pendingRemovals: Map<string, PendingRelayRemoval>;
}
/**
 * One Relay row this host stopped but has not deleted there yet.
 *
 * The share is gone from this host already, and the row and the token that deletes it are kept only
 * until the Relay answers: a stop that landed while the Relay was unreachable is delivered at the
 * next contact instead of being lost until the row's expiry.
 */
export interface PendingRelayRemoval {
    readonly relay: string;
    readonly shareId: string;
    readonly gatewayToken: string;
}
/**
 * Read this host's owner state file, creating an empty state when it does not exist.
 *
 * Every stored share names the Relay it was published to, and a record without one is invalid: this
 * build has no configured address to read such a share against, and LiveShare has never shipped a
 * state file that omits the field.
 * @param file - Owner state file path.
 * @returns The loaded saved address, instance id, shares, and owed Relay rows.
 */
export declare function loadOwnerState(file: string): LoadedOwnerState;
/**
 * Write this host's owner state, serialized per file so the newest content lands last.
 * @param file - Owner state file path.
 * @param relayUrl - Relay override currently in force, when the owner set one.
 * @param instanceId - Identity of this owner instance.
 * @param shares - Shares to store.
 * @param pendingRemovals - Relay rows this host still owes a deletion.
 * @returns Completion of this write, including any failure the caller reports.
 */
export declare function writeOwnerState(file: string, relayUrl: string | undefined, instanceId: string, shares: ReadonlyMap<string, OwnerShare>, pendingRemovals: ReadonlyMap<string, PendingRelayRemoval>): Promise<void>;
/**
 * Read the mode a state file stored, mapping the retired `approval` spelling to `selected`.
 * @param mode - Access mode as the state file recorded it.
 * @returns The mode this build's runtime and wire surfaces use.
 */
export declare function storedAccessMode(mode: 'open' | 'selected' | 'approval'): 'open' | 'selected';
//# sourceMappingURL=owner-persistence.d.ts.map