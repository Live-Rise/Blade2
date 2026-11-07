/** Web-only host plugin: owner sign-in routes, the share-management Remote, and gateway lifetime. */
import type { Context } from '@deepseek-ai/cordis';
import type { OwnerShare } from './owner-types.ts';
import type { ForkShareState } from './fork-policy.ts';
import { type IdentityActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
export declare const name = "liveshare-share-host";
export declare const inject: string[];
export { requesterStillAdmitted } from './relay-authority.ts';
export { gatewayWorkerCommand, refreshedShareTitle } from './tunnel-supervision.ts';
export { activateSession } from './source-access.ts';
export type { SourceAccessApplier } from './source-access.ts';
export type { OwnerShare } from './owner-types.ts';
/**
 * Host plugin configuration: the owner identity shares are created under and the owner-local files that
 * retain shares and the credential across restarts.
 *
 * The public Relay address is not configuration. The owner saves it in the Share dialog, where it
 * persists in the owner state file, so a profile that switches this bundle on without an address still
 * loads: its sharing surfaces report that no address is set until the owner saves one.
 */
export interface Config {
    /** Provider subject ids allowed to view every share created by this host. */
    readonly allowedSubjectIds?: readonly string[];
    /** Authenticated owner identity used to scope people-directory searches. */
    readonly ownerIdentity?: IdentityActor;
    /** Owner identity provider used when `ownerIdentity` is assembled from fields. */
    readonly ownerIdentityProvider?: string;
    /** Owner identity tenant used when `ownerIdentity` is assembled from fields. */
    readonly ownerIdentityTenant?: string;
    /** Owner identity subject used when `ownerIdentity` is assembled from fields. */
    readonly ownerIdentitySubject?: string;
    /** Owner identity display name used when `ownerIdentity` is assembled from fields. */
    readonly ownerIdentityDisplayName?: string;
    /** Owner-local JSON file retaining share metadata and gateway tokens across host restarts. */
    readonly ownerStateFile?: string;
    /**
     * Owner credential file. Defaults beside the owner state file; it holds the Relay-issued
     * credential, which is why it is written with owner-only permissions and never reported.
     */
    readonly ownerCredentialFile?: string;
}
/**
 * The share state one viewer request is decided against, read from this host's published share.
 *
 * The Relay admits viewers before a request arrives, so admission is a fact here and the published
 * share supplies the rest. The owner's mode is part of it: the reader routes a prompt naming the share's
 * own Session to the source-session path only when this says `write`, and installs a child with this
 * host's bounded write only for a write share — absent, both answer as reading only.
 * @param share - the share this host published to the Relay.
 * @returns the state the viewer reader decides a fork, a child prompt, or a source prompt against.
 */
export declare function publishedShareState(share: OwnerShare): ForkShareState;
/**
 * Register the owner sign-in routes and the share-management Remote, and enforce current grants on
 * external Session input.
 * @param ctx - Host context carrying Connection and optional execution services.
 * @param config - Relay and owner-state configuration.
 * @returns Async disposer that stops the tunnel worker and releases share access.
 */
export declare function apply(ctx: Context, config: Config): Promise<() => Promise<void>>;
//# sourceMappingURL=share-host.d.ts.map