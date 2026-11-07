/**
 * The Relay's statement of one share's access, and the reads that adopt it.
 *
 * A share's ACL, tier, and expiry belong to the Relay once it states them, so this module holds the
 * last state frame it reported, fences each read against the generation of the share it was for, and
 * decides whether a viewer still holds the admission a frame describes.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/relay-authority
 */
import type { IdentityActor, IdentityTriple, ShareAccessStateFrame, ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import type { ForkRegistry } from './fork-registry.ts';
import { type ViewerSessionMode } from './fork-policy.ts';
import type { OwnerShare } from './owner-types.ts';
import type { ViewerReadService } from './viewer-read.ts';
/** What one authoritative Relay read of a share produced. */
export interface RelayAuthorityRead {
    readonly ok: boolean;
    readonly accessState?: ShareAccessStateFrame;
}
/**
 * Whether one requester still holds access to one share this host publishes.
 *
 * The Relay admits a request before it reaches this host, so admission is a fact for the request that
 * carried the input; input admitted earlier has to be re-checked, because the grant it relied on may have
 * ended while it waited in the queue. The rule is the Relay's own admission rule: an open share admits the
 * whole organization once its own ACL is empty, and every share admits the subjects its ACL names — by
 * subject id or by canonical identity key.
 * @param share - Share the input arrived through, as this host holds it.
 * @param requester - Identity the turn's origin recorded.
 * @returns True while that identity still holds access.
 */
export declare function requesterStillAdmitted(share: Pick<OwnerShare, 'accessMode' | 'allowedSubjectIds'>, requester: {
    readonly providerId: string;
    readonly tenantId: string;
    readonly subjectId: string;
}): boolean;
/** Host state one Relay authority read and the frames it adopts read. */
export interface RelayAuthorityDeps {
    /** Shares this host publishes, which a frame and a read both update or verify. */
    readonly ownerShares: Map<string, OwnerShare>;
    /** Whether the host has been disposed. */
    readonly disposed: () => boolean;
    /** Children this host created, whose grants a narrowed frame ends. */
    readonly forks: ForkRegistry;
    /** This host's own identity, used to read subject ids as identities. */
    readonly ownerIdentity: IdentityActor | undefined;
    /** Subjects this deployment admits to every share. */
    readonly allowedSubjectIds: readonly string[];
    /** The viewer reader, which releases work admitted against a superseded snapshot. */
    readonly viewerRead: () => ViewerReadService;
    /** Tell remote access that reader answers may have changed. */
    readonly accessChanged: () => void;
    /** Stop the running turn one share may no longer drive. */
    readonly cancelShareTurn: (shareId: string, revokedSubject?: string) => void;
    /** Stop child work whose grant is narrowed or removed. */
    readonly cancelChildTurns: (shareId: string, viewer?: ShareRpcActor) => void;
    /** Arm or re-arm one share's expiry cancellation. */
    readonly scheduleShareExpiry: (share: OwnerShare) => void;
    /** Reconcile one Session's sandbox after its shares' tiers, deadlines, or admissions changed. */
    readonly reconcileSourceAccess: (sessionId: string) => void;
}
/** Reads and adopted states of the Relay's authority over this host's shares. */
export interface RelayAuthority {
    /**
     * Whether one Relay admission still stands for one viewer.
     * @param shareId - Share the viewer was admitted to.
     * @param viewer - Identity the Relay asserted.
     * @returns True while that identity is still admitted.
     */
    viewerAdmitted(shareId: string, viewer: ShareRpcActor): boolean;
    /**
     * Forget one share's held authority after the worker reconnected it.
     * @param shareId - Share the worker reconnected.
     */
    forgetRelayAccess(shareId: string): void;
    /**
     * Fence off one share's held authority and release the work admitted against it.
     * @param shareId - Share whose authority ended.
     */
    invalidateRelayAccess(shareId: string): void;
    /**
     * Adopt one Relay's statement of a share's access.
     * @param shareId - Share the frame describes.
     * @param state - Parsed access-state frame.
     */
    applyRelayAccess(shareId: string, state: ShareAccessStateFrame): void;
    /**
     * Read one share's authority from its Relay and adopt what it states.
     * @param share - Share to read.
     * @param signal - Cancellation of the forwarded request.
     * @returns The snapshot that was adopted, or an unavailable read.
     */
    readRelayAuthority(share: OwnerShare, signal?: AbortSignal): Promise<RelayAuthorityRead>;
    /**
     * Refresh one share's authority at request ingress, throwing when it cannot be read.
     * @param shareId - Share to refresh.
     * @param signal - Cancellation of the forwarded request.
     */
    refreshRelayAccess(shareId: string, signal?: AbortSignal): Promise<void>;
    /**
     * The tier this host may still drive one share with, or undefined when it may not.
     * @param shareId - Share the execution arrived through.
     * @returns The tier, or undefined when the share is gone, past its deadline, or not yet reported.
     */
    drivableModeOf(shareId: string): ViewerSessionMode | undefined;
}
/**
 * Whether one adopted frame can have ended access some viewer held under the previous one.
 *
 * The Relay admits the whole organization while the ACL is empty and the share is open; once either
 * changes, only the named subjects remain. A subject the previous ACL named and the new one does not is a
 * viewer whose grant ended.
 * @param previous - The frame this host held before.
 * @param next - The frame it just adopted.
 * @returns True when someone the previous frame admitted may no longer be admitted.
 */
export declare function admissionNarrowed(previous: ShareAccessStateFrame, next: ShareAccessStateFrame): boolean;
/**
 * Read one granted subject as an identity, when the owner named a bare subject id.
 * @param subjectId - Subject id or canonical identity key the owner named.
 * @param ownerIdentity - This host's own identity, which supplies the provider and tenant.
 * @returns The identity, or undefined when the owner named no identity this host can read.
 */
export declare function identityForGrantedSubject(subjectId: string, ownerIdentity: IdentityActor | undefined): IdentityTriple | undefined;
/**
 * Build the reader of the Relay's authority over this host's shares.
 * @param deps - Shares, viewer reader, and the work a narrowed grant ends.
 * @returns Reads and adopted states of the Relay's authority.
 */
export declare function createRelayAuthority(deps: RelayAuthorityDeps): RelayAuthority;
//# sourceMappingURL=relay-authority.d.ts.map