/**
 * Worker supervision for one owner host: registration replay, heartbeats, expiry timers, and rows owed.
 *
 * The gateway worker process carries every share this host publishes, so this module owns its
 * lifetime: the commands sent over IPC, the settlements that resolve a pending registration, the
 * timers that re-register or remove a share, and the credential state each registration presents.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/tunnel-supervision
 */
import type { Context } from '@deepseek-ai/cordis';
import type { IdentityActor, TunnelShareSpec } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import type { ClaimOutcome, OwnerIdentityStatus } from '../owner-identity.ts';
import type { RelayConnectivityView, RelayPublishView } from '../share-api-types.ts';
import type { ViewerSessionMode } from './fork-policy.ts';
import type { PendingRelayRemoval } from './owner-persistence.ts';
import { type StoredOwnerCredential } from './owner-credential-file.ts';
import type { OwnerShare } from './owner-types.ts';
import type { RelayAuthority } from './relay-authority.ts';
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import type { ViewerReadService } from './viewer-read.ts';
import { type TunnelWorkerInput } from './tunnel-worker.ts';
/** Fields one gateway registration needs from a share, before it is stored as an OwnerShare. */
type ShareRegistrationSource = Pick<OwnerShare, 'shareId' | 'sessionId' | 'sessionTitle' | 'accessMode' | 'relay' | 'gatewayToken' | 'allowedSubjectIds'> & {
    readonly expiresAt?: string;
    readonly allowsFork: boolean;
    readonly sessionMode: ViewerSessionMode;
};
/** Node process and argv that start the owner gateway worker. */
export interface TunnelProcessCommand {
    readonly executable: string;
    readonly args: readonly string[];
}
/**
 * Resolve the gateway worker entry beside the running host module.
 * A source checkout loads this module from `src/host/`, while the bundled host is the package's
 * `lib/index.js` entry and ships the worker as `lib/owner-gateway/cli.js`.
 * @param moduleUrl - `import.meta.url` of the running host module.
 * @returns Node executable and argv that start the owner gateway worker.
 */
export declare function gatewayWorkerCommand(moduleUrl: string): TunnelProcessCommand;
/** Owner state, credential, and worker channels one host's registration and timers read. */
export interface TunnelSupervisionDeps {
    /** Host context, read for the optional title services the reconcile step uses. */
    readonly ctx: Context;
    /** Shares this host publishes; registration replay walks them. */
    readonly ownerShares: Map<string, OwnerShare>;
    /** Relay rows this host stopped but has not deleted. */
    readonly pendingRemovals: Map<string, PendingRelayRemoval>;
    /** This owner instance's id, carried by every registration. */
    readonly instanceId: string;
    /** Subjects this deployment admits to every share. */
    readonly allowedSubjectIds: readonly string[];
    /** Owner identity fields every registration carries, when this host has an identity. */
    readonly ownerContext: {
        readonly ownerProviderId?: string;
        readonly ownerTenantId?: string;
    };
    /** This host's own identity, used when a credential carries no name. */
    readonly ownerIdentity: IdentityActor | undefined;
    /** Relay authority reads and adopted state. */
    readonly relay: RelayAuthority;
    /** The viewer reader, answered for forwarded requests. */
    readonly viewerRead: () => ViewerReadService;
    /** Tell remote access that reader answers may have changed. */
    readonly accessChanged: () => void;
    /** Stop whatever turn one share's Session is running. */
    readonly cancelShareTurn: (shareId: string) => void;
    /** Reconcile one Session's sandbox after a share stopped or expired. */
    readonly reconcileSourceAccess: (sessionId: string) => void;
    /** Stop child work whose grant is narrowed or removed. */
    readonly cancelChildTurns: (shareId: string, viewer?: ShareRpcActor) => void;
    /** Whether this host has been disposed. */
    readonly disposed: () => boolean;
    /** Owner credential file, written when a Relay reports a horizon. */
    readonly ownerCredentialFile: string;
    /** Credential this host holds now. */
    readonly storedCredential: () => StoredOwnerCredential | undefined;
    /** Replace the credential this host holds. */
    readonly setStoredCredential: (credential: StoredOwnerCredential) => void;
    /** Persist this host's owner state. */
    readonly persistOwnerState: () => Promise<void>;
}
/** Worker lifecycle, registration replay, credential state, and timers for one owner host. */
export interface TunnelSupervision {
    /**
     * Send one command to the gateway worker, starting it when it is not running.
     * @param command - Command the worker carries out.
     */
    send(command: TunnelWorkerInput): void;
    /**
     * Wait for one registration to settle.
     * @param shareId - Share the registration named.
     * @param timeoutMs - Longest wait before the registration is reported as timed out.
     * @returns The worker's settlement for that registration.
     */
    waitForShare(shareId: string, timeoutMs: number): Promise<{
        ok: boolean;
        error?: string;
    }>;
    /**
     * Build the worker's registration for one owner share.
     * @param share - Share or throwaway registration to publish.
     * @param credential - Credential captured by the operation.
     * @returns The registration payload.
     */
    shareSpec(share: ShareRegistrationSource, credential: StoredOwnerCredential | undefined): TunnelShareSpec;
    /**
     * Arm the cancellation that stops a share's running turn the moment the share expires.
     * @param share - Share to arm or re-arm.
     */
    scheduleShareExpiry(share: OwnerShare): void;
    /**
     * Disarm the deadline of a share that is no longer published.
     * @param shareId - Share whose deadline is disarmed.
     */
    clearShareExpiry(shareId: string): void;
    /**
     * Delete the Relay rows this host still owes.
     */
    flushPendingRemovals(): Promise<void>;
    /** Arm the next attempt for the rows still owed, or disarm when none are left. */
    armPendingRemovalRetry(): void;
    /**
     * Delete one share's row from the Relay that carries it.
     * @param relay - Relay base URL the row belongs to.
     * @param shareId - Share whose row is deleted.
     * @param gatewayToken - Management token the Relay authorizes the deletion with.
     * @returns True when the Relay removed the row, or already carried none.
     */
    deleteRelayRow(relay: string, shareId: string, gatewayToken: string): Promise<boolean>;
    /**
     * Refresh every share's recorded title from the Session's own title fold.
     */
    reconcileShareTitles(): Promise<void>;
    /**
     * Per-Relay facts this host has learned.
     * @param relay - Relay address the facts describe.
     * @returns The facts for that normalized address.
     */
    factsFor(relay: string): {
        supportsCredentials?: boolean;
        claim?: ClaimOutcome;
    };
    /**
     * Ask the Relay whether it binds owner identity, and what horizon it grants the credential held.
     * @param relay - Relay base URL to probe.
     */
    probeRelayCredentialSupport(relay: string): Promise<void>;
    /**
     * Bind this host's existing shares to its verified identity.
     * @param relay - Normalized Relay address the credential was issued for.
     * @param credential - Credential captured by the operation, absent when the host is unverified.
     * @returns How many shares this pass bound and how many were already bound to this identity.
     */
    claimOwnedShares(relay: string, credential: string | undefined): Promise<ClaimOutcome>;
    /**
     * Credential currently usable for one Relay address.
     * @param relay - Relay address the credential was bound to.
     * @param credential - Credential this host holds.
     * @returns The credential, or undefined when this host does not judge it usable there.
     */
    credentialForRelay(relay: string, credential: StoredOwnerCredential | undefined): string | undefined;
    /**
     * Credential stored for one Relay address, whether or not this host still judges it unexpired.
     * @param relay - Relay address the credential was bound to.
     * @returns The credential, or undefined when none is stored for that address.
     */
    claimableCredentialForRelay(relay: string): string | undefined;
    /**
     * Register this host's shares for one Relay address again when they are not connected.
     * @param relay - Normalized Relay address the credential was issued for.
     * @param credential - Credential just committed for that address.
     */
    reregisterOfflineShares(relay: string, credential: StoredOwnerCredential): void;
    /**
     * Check that one Relay answers at all.
     * @param relay - Relay address to check.
     * @returns The anonymous connectivity result.
     */
    probeRelayConnectivity(relay: string): Promise<RelayConnectivityView>;
    /**
     * Register one throwaway share to check whether this host may publish.
     * @param relay - Relay address to probe.
     * @param share - Throwaway registration to carry.
     * @returns The Relay's decision about this host's credential.
     */
    probeRelayPublish(relay: string, share: Omit<TunnelShareSpec, 'shareId'>): Promise<RelayPublishView>;
    /**
     * Owner identity state for one Relay address, without exposing the credential itself.
     * @param relay - Relay address to report.
     * @param extra - Renewal or claim facts to include.
     * @param credential - Credential this host holds.
     * @returns The owner identity status.
     */
    ownerIdentityStatus(relay: string, extra: {
        readonly renewed?: boolean;
        readonly claim?: ClaimOutcome;
    }, credential: StoredOwnerCredential | undefined): OwnerIdentityStatus;
    /**
     * Stop the worker and release every timer and pending registration this module holds.
     * @param closeViewer - Releases the viewer reader at its point in the shutdown order.
     */
    dispose(closeViewer: () => void): Promise<void>;
}
/**
 * Title one share should adopt after reading its Session.
 * @param recorded - Title stored on the share when it was created or last refreshed.
 * @param current - Title the Session's own title fold reports.
 * @returns The replacement title, or undefined when the Session reports none or the same one.
 */
export declare function refreshedShareTitle(recorded: string, current: unknown): string | undefined;
/**
 * Build the worker supervisor for one owner host.
 * @param deps - Shares, credential state, viewer reader, and persistence.
 * @returns Worker lifecycle, registration replay, credential state, and timers.
 */
export declare function createTunnelSupervision(deps: TunnelSupervisionDeps): TunnelSupervision;
export {};
//# sourceMappingURL=tunnel-supervision.d.ts.map