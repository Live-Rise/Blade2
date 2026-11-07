/**
 * The twelve owner share-management operations behind the `liveshareShares` Remote.
 *
 * Each method owns one operation's business order; the Remote service exposes it without a second
 * implementation. Failures are thrown as `RemoteError` values with a `liveshareShares/` code, which
 * the Gateway carries to the Client unchanged.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/share-operations-impl
 */
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import type { OwnerRelayState } from '../share-api-types.ts';
import type { ForkRegistry } from './fork-registry.ts';
import type { StoredOwnerCredential } from './owner-credential-file.ts';
import type { PendingRelayRemoval } from './owner-persistence.ts';
import type { RelayAuthority } from './relay-authority.ts';
import type { OwnerShare } from './owner-types.ts';
import type { SourceAccessApplier } from './source-access.ts';
import type { OwnerShareOperations } from './share-operations.ts';
import type { TunnelSupervision } from './tunnel-supervision.ts';
import type { ViewerDeclarationsSurface } from './viewer-declarations.ts';
import type { ViewerReadService } from './viewer-read.ts';
/** Host state the twelve operations read and change. */
export interface OwnerShareOperationDeps {
    /** Shares this host publishes. */
    readonly ownerShares: Map<string, OwnerShare>;
    /** Relay rows this host stopped but has not deleted. */
    readonly pendingRemovals: Map<string, PendingRelayRemoval>;
    /** Subjects this deployment admits to every share. */
    readonly allowedSubjectIds: readonly string[];
    /** Children this host created. */
    readonly forks: ForkRegistry;
    /** The viewer reader, closed and re-checked by lifecycle changes. */
    readonly viewerRead: ViewerReadService;
    /** The declaration surface whose live offers a capability selection is accepted against. */
    readonly viewerDeclarations: ViewerDeclarationsSurface;
    /** Reads and adopted states of the Relay's authority over this host's shares. */
    readonly relay: RelayAuthority;
    /** Worker registration, expiry timers, and persistence channels. */
    readonly tunnel: TunnelSupervision;
    /** Current effective Relay address; absent while the owner has saved none. */
    readonly effectiveRelay: () => string | undefined;
    /** Configured and effective Relay addresses, as the owner UI reads them. */
    readonly relayState: () => OwnerRelayState;
    /** Set the owner's Relay override, which also sets the effective address. */
    readonly selectRelayAddress: (override: string | undefined) => void;
    /** Bump the revision that fences owner operations begun under the previous address. */
    readonly bumpRelayRevision: () => void;
    /** Credential this host holds now. */
    readonly storedCredential: () => StoredOwnerCredential | undefined;
    /** Persist this host's owner state. */
    readonly persistOwnerState: () => Promise<void>;
    /** Install the access layer one share's source Session needs. */
    readonly applySourceAccess: SourceAccessApplier;
    /** Whether one share's source Session carries a usable confined layer. */
    readonly sourceConfined: (shareId: string) => boolean;
    /** Tell remote access that reader answers may have changed. */
    readonly accessChanged: () => void;
    /** Stop whatever turn one share's Session is running. */
    readonly cancelShareTurn: (shareId: string) => void;
    /** Reconcile one Session's sandbox after a share stopped. */
    readonly reconcileSourceAccess: (sessionId: string) => void;
    /** Stop child work whose grant is narrowed or removed. */
    readonly cancelChildTurns: (shareId: string, viewer?: ShareRpcActor) => void;
}
/**
 * Build the twelve owner share-management operations for one host.
 * @param deps - Shares, Relay authority, worker channel, persistence, and lifecycle callbacks.
 * @returns The operations the Remote service calls.
 */
export declare function createOwnerShareOperations(deps: OwnerShareOperationDeps): OwnerShareOperations;
//# sourceMappingURL=owner-share-operations.d.ts.map