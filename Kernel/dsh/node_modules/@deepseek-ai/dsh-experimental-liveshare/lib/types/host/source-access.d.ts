/**
 * The confined access layer one share's source Session carries, and the turn control around it.
 *
 * The layer is registered into the Session's own scope and confines the turns a share viewer starts while
 * any share over the Session is active; the owner's own turns keep the deployment's tools. It belongs to
 * the Session rather than to one share, so several shares over it share one installation, and the host
 * releases it once no share is left. Stopping a share stops whatever turn the Session is running, with
 * the owner's own cancellation, without asking which turn or which requester it was.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/source-access
 */
import type { Context } from '@deepseek-ai/cordis';
import { type Session } from '@deepseek-ai/dsh-session';
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import { type ForkToolPaths, type SourceSessionAccessInstallation } from './fork-enforcement.ts';
import type { ForkToolExecution } from './fork-tool-guard.ts';
import type { ForkRegistry } from './fork-registry.ts';
import type { RelayAuthority } from './relay-authority.ts';
/** Owner share shape this module reads; the host owns the full record. */
import type { OwnerShare } from './owner-types.ts';
/** Where one source-access attempt came from; named in its diagnostic line. */
export type WriteAccessTrigger = 'publish' | 'restore' | 'mode' | 'reader' | 'prompt';
/** Why one source-access installation did not happen: the guard that refused, as a stable code. */
type WriteAccessRefusal = 'no-live-session' | 'no-live-context' | 'no-session-scope' | 'no-filesystem' | 'no-tool-registry' | 'no-sandbox-policy' | 'no-workspace-root' | 'no-session-cwd' | 'workspace-unresolved' | 'share-gone' | 'share-reassigned' | 'share-mode-changed' | 'share-inactive';
/** What one source-access attempt did. */
export type WriteAccessOutcome = 
/** This attempt installed the layer. */
{
    readonly kind: 'installed';
    readonly installation: SourceSessionAccessInstallation;
}
/** A usable layer was already recorded, by this attempt's caller or another share's. */
 | {
    readonly kind: 'kept';
    readonly installation: SourceSessionAccessInstallation;
}
/** A guard refused, and named itself in the diagnostic. */
 | {
    readonly kind: 'refused';
    readonly reason: WriteAccessRefusal;
};
/** Installs the access layer one share's source Session needs. */
export type SourceAccessApplier = (share: OwnerShare, ready: {
    readonly context: Context;
    readonly session: Session;
} | undefined, trigger: WriteAccessTrigger) => Promise<WriteAccessOutcome>;
/**
 * Bring one Session's Agent up through the composition's own activation path and wait for it.
 *
 * The Session is resolved or resumed exactly as an ordinary Host request resolves it — preset
 * composition, model-selection installation, then publication — so every plugin that installs itself
 * from the `agent/created` announcement has run by the time this settles. That is what makes this host
 * safe to call before it drives a Session whose Agent may have been activated in an earlier process.
 *
 * The capability is the merge-declared `agent` lookup: its resolver belongs to the composition, and the
 * API Session controller points it at the same activation path its own endpoints use. Reading it here
 * keeps this bundle free of any dependency on those endpoints, and a deployment that installs no such
 * lookup is refused loudly rather than silently treated as activated.
 * @param ctx - Host context carrying the Typert lookup registry.
 * @param sessionId - Session to resolve or resume.
 * @throws when this deployment installs no agent lookup, or the lookup activates no Agent.
 */
export declare function activateSession(ctx: Context, sessionId: string): Promise<void>;
/** Host state the source Session's access layer reads. */
export interface SourceAccessDeps {
    /** Host context carrying the optional Agent, filesystem, tool, and sandbox services. */
    readonly ctx: Context;
    /** Shares this host publishes. */
    readonly ownerShares: Map<string, OwnerShare>;
    /** Children this host created, whose turns a narrowed grant ends. */
    readonly forks: ForkRegistry;
    /** Reads of the Relay's authority over one share. */
    readonly relay: RelayAuthority;
    /** Whether this host has been disposed. */
    readonly disposed: () => boolean;
    /** Tell remote access that reader answers may have changed. */
    readonly accessChanged: () => void;
    /**
     * Whether one tool execution belongs to a turn a share viewer started.
     *
     * The source layer confines those turns and leaves an owner-started one on the deployment's tools, so
     * this is the per-execution authority both the guard and the replacements read.
     */
    readonly confinedExecution: (execution: ForkToolExecution) => boolean;
}
/** The confined access layer and turn control one host's source Sessions carry. */
export interface SourceAccess {
    /**
     * Install the access layer one share's source Session needs.
     * @param share - Share the layer is being installed for.
     * @param ready - The announced agent's own scope and session, when the caller has them.
     * @param trigger - Where this attempt came from, named in the diagnostic a refusal logs.
     * @returns What this attempt did.
     */
    apply(share: OwnerShare, ready?: {
        readonly context: Context;
        readonly session: Session;
    }, trigger?: WriteAccessTrigger): Promise<WriteAccessOutcome>;
    /**
     * Whether one share's source Session carries a usable confined access layer right now.
     * @param shareId - Share the request arrived through.
     * @returns True only while the layer is installed and confined.
     */
    confined(shareId: string): boolean;
    /**
     * Why one share's source Session is not confined for driving, or undefined while it is.
     * @param shareId - Share the question is asked about.
     * @returns The refusal code, or undefined while the source Session is confined for this share.
     */
    confinementRefusal(shareId: string): string | undefined;
    /**
     * The recorded installation's own refusal code for one share's Session.
     * @param shareId - Share whose source Session is asked about.
     * @returns The refusal, or undefined when none was recorded.
     */
    refusalOf(shareId: string): string | undefined;
    /**
     * Stop child work whose grant is being narrowed or removed.
     * @param shareId - Share whose children are stopped.
     * @param viewer - Only this viewer's children, when one grant ended.
     */
    cancelChildTurns(shareId: string, viewer?: ShareRpcActor): void;
    /**
     * The recorded child may use only tools its share currently permits.
     * @param childSessionId - Child whose tool is being judged.
     * @param tool - Tool name the call names.
     * @returns True while the child may run that tool.
     */
    childToolAuthorized(childSessionId: string, tool: string): boolean;
    /**
     * Stop whatever turn one share's Session is running, when the share stops driving it.
     *
     * The host records only whether a turn was viewer-started, not which viewer admitted it, so it cannot
     * tell a revoked viewer's turn from the owner's: a share that ends its grant stops the running turn
     * itself.
     * @param shareId - Share whose capability just ended.
     */
    cancelShareTurn(shareId: string): void;
    /**
     * Make the Session's layer match its shares after one of them stopped, expired, or changed tier.
     *
     * A Session whose shares are all inactive loses its layer, which restores the Session's own tools; a
     * Session whose shares no longer need the bounded write is re-installed read-only.
     * @param sessionId - Session whose published shares changed.
     */
    reconcileSessionAccess(sessionId: string): void;
    /**
     * Prepare one share's source Session for driving, waiting only for the activation it needs.
     * @param sessionId - Session one published share names.
     * @param trigger - Where this preparation came from.
     */
    prepareSourceSession(sessionId: string, trigger: WriteAccessTrigger): Promise<void>;
    /**
     * Start preparing one share's source Session, at most once at a time, without waiting for it.
     * @param shareId - Share one reader just asked about.
     */
    prepareSourceForReader(shareId: string): void;
    /**
     * The paths one tool call names, as every enforcement installation in this host reads them.
     * @param execution - The call being judged.
     * @returns The paths it names, or nothing when they cannot be established.
     */
    forkToolPaths(execution: ForkToolExecution): ForkToolPaths;
}
/**
 * Build the confined access layer for this host's source Sessions.
 * @param deps - Shares and the Relay authority the layer reads.
 * @returns The access layer and turn control.
 */
export declare function createSourceAccess(deps: SourceAccessDeps): SourceAccess;
export {};
//# sourceMappingURL=source-access.d.ts.map