/**
 * The owner's share management as one Typert Remote service.
 *
 * Every method delegates to the one {@link OwnerShareOperations} implementation that owns each
 * share-management side effect, so the owner page gains a Remote surface without a second copy.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/share-remote
 */
import type { Context } from '@deepseek-ai/cordis';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { CreateShareRequest, CreatedShare, OwnerIdentityRequest, OwnerIdentityStatus, OwnerRelayState, OwnerShareList, RelayConnectivityView, RelayPublishView, SetRelayRequest, SetShareCapabilitiesRequest, ShareAudit, ShareAuditRequest, ShareCapabilities, ShareCapabilityOffers, SharePermissions, StopShareRequest, StopShareResult, TestPublishRequest, UpdateSharePermissionsRequest } from '../share-api-types.ts';
import type { OwnerShareOperations } from './share-operations.ts';
/**
 * Owner-only Remote surface over the share-management operations.
 *
 * The Gateway decides each call through remote access, so a `share-viewer` Peer is refused before a
 * method runs; the owner Peer reaches the one implementation that owns each share-management side
 * effect. `create` takes its Session as the `sessionId` of its single request object, so the Session
 * resource kind's default resolver names that Session for the call without a reader declared here.
 */
export declare class LiveshareShares extends TypertRemoteService {
    private readonly operations;
    /**
     * @param ctx - Host context that owns the share state and the worker channel.
     * @param operations - Owner operations implemented in the Host's `apply()` closure.
     */
    constructor(ctx: Context, operations: OwnerShareOperations);
    /**
     * Publish one Session and store the share.
     * @param request - Session, title, access, lifetime, and both switches the owner chose.
     * @returns The created share's link and stored values.
     */
    create(request: CreateShareRequest): Promise<CreatedShare>;
    /**
     * Refresh titles and Liveshare server authority, then list this host's shares.
     * @returns Every share this host publishes.
     */
    list(): Promise<OwnerShareList>;
    /**
     * Stop one share locally, revoke its forks, and delete its row on the Liveshare server when reachable.
     * @param request - Share to stop.
     * @returns Whether the row on the Liveshare server still needs a retry.
     */
    stop(request: StopShareRequest): Promise<StopShareResult>;
    /**
     * Change one share's tier, fork switch, or both.
     * @param request - Share and the switches to change.
     * @returns The permissions the share now carries.
     */
    updatePermissions(request: UpdateSharePermissionsRequest): Promise<SharePermissions>;
    /**
     * Replace one share's exposed plugin endpoints.
     * @param request - Share and the selection to store.
     * @returns The capability selection the share now carries.
     */
    setCapabilities(request: SetShareCapabilitiesRequest): Promise<ShareCapabilities>;
    /**
     * Read the endpoints plugins currently offer.
     * @returns The live offer list.
     */
    capabilities(): ShareCapabilityOffers;
    /**
     * Read the Liveshare server address the owner saved.
     * @returns The current Liveshare server state, with both address fields absent while none is saved.
     */
    relay(): OwnerRelayState;
    /**
     * Choose the Liveshare server address later shares publish to.
     * @param request - Normalized address or null to clear the saved one.
     * @returns The Liveshare server state after the change.
     */
    setRelay(request: SetRelayRequest): Promise<OwnerRelayState>;
    /**
     * Report this host's owner identity for one Liveshare server address.
     * @param request - Address to report, or the effective one when absent.
     * @returns The owner identity status, without the credential.
     */
    identity(request: OwnerIdentityRequest): Promise<OwnerIdentityStatus>;
    /**
     * Read the Liveshare server's durable audit trail for one share.
     * @param request - Share whose audit is read.
     * @returns The audit events, or a structured failure when the Liveshare server cannot answer.
     */
    audit(request: ShareAuditRequest): Promise<ShareAudit>;
    /**
     * Check that the effective Liveshare server answers at all.
     * @returns The anonymous connectivity result.
     */
    testRelay(): Promise<RelayConnectivityView>;
    /**
     * Register one throwaway share to check whether this host may publish.
     * @param request - Session the throwaway registration names.
     * @returns The Liveshare server's decision about this host's credential.
     */
    testPublish(request: TestPublishRequest): Promise<RelayPublishView>;
}
//# sourceMappingURL=share-remote.d.ts.map