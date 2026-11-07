/**
 * The owner sign-in HTTP surface: the four same-origin routes the browser still posts to.
 *
 * Share management is the `liveshareShares` Remote, so these routes only begin and redeem the
 * owner's Relay authorization and store the credential it returns; every share-management side
 * effect reaches the Gateway through the Remote instead.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/owner-http-routes
 */
import type { Context } from '@deepseek-ai/cordis';
import { OwnerAuthorizations } from './owner-authorization.ts';
import { type StoredOwnerCredential } from './owner-credential-file.ts';
import type { TunnelSupervision } from './tunnel-supervision.ts';
/** One owner operation's captured Relay, revision, and host instance. */
export interface OwnerOperation {
    /** Address in force when the operation was captured; absent while the owner has saved none. */
    readonly relay: string | undefined;
    readonly relayRevision: number;
    readonly instanceKey: string;
}
/** Host state the owner sign-in routes read. */
export interface OwnerHttpRouteDeps {
    /** Host Connection service the routes are registered on. */
    readonly connection: Context['connection'];
    /** Worker registration, credential state, and Relay probes. */
    readonly tunnel: TunnelSupervision;
    /** Capture the Relay, revision, and instance an operation began under. */
    readonly captureOwnerOperation: () => OwnerOperation;
    /** Whether one captured operation still describes this host's Relay and instance. */
    readonly ownerOperationCurrent: (operation: OwnerOperation) => boolean;
    /** Owner authorizations this host began and is waiting to redeem. */
    readonly ownerAuthorizations: OwnerAuthorizations;
    /** Owner credential file, written when the Relay issues a credential. */
    readonly ownerCredentialFile: string;
    /** Credential this host holds now. */
    readonly storedCredential: () => StoredOwnerCredential | undefined;
    /** Replace the credential this host holds. */
    readonly setStoredCredential: (credential: StoredOwnerCredential) => void;
    /** Whether this host has been disposed. */
    readonly disposed: () => boolean;
}
/**
 * Register the owner sign-in routes on this host's Connection.
 * @param deps - Connection, worker channel, and owner credential state.
 */
export declare function registerOwnerHttpRoutes(deps: OwnerHttpRouteDeps): void;
//# sourceMappingURL=owner-http-routes.d.ts.map