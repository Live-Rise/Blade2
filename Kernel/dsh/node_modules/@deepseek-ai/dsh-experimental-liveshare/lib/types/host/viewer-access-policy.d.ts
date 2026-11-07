/**
 * Remote access policy deciding what one share reader may call on this owner Host.
 *
 * The Gateway decides every call that speaks for a `share-viewer` Peer, and every Session notification it
 * forwards to one, from the bindings this policy lists; the Session and Workspace controllers filter list
 * results through the same bindings, and every reader page opens with the snapshot computed from them. A
 * served reader holds the global read role, read roles on the shared Session and on the forks it created
 * through the same share, and the Workspace role on each Workspace holding one of those Sessions; it holds
 * nothing while the share does not serve it. Attachment upload rides the read role, so every Session the
 * reader can see is a Session it may attach files to. Conditions on the fork and write roles, and the
 * policy's `explain`, give the refusal codes the reader page already classifies.
 *
 * Every rule reads in-memory share state synchronously. Work that needs an await (Relay refresh,
 * activating a cold Session, fork identity and replay) stays in the viewer reader before dispatch, which
 * reads the same write rules through {@link readerWriteRefusal}.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-access-policy
 */
import type { RemoteAccessAction, RemoteAccessPolicy } from '@deepseek-ai/dsh-api-remote-access/types';
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import type { Workspace } from '@deepseek-ai/dsh-workspace/types';
import type { ForkRegistry } from './fork-registry.ts';
import { type ForkShareState } from './fork-policy.ts';
import type { ViewerCapabilityGroup } from './viewer-declarations.ts';
import type { ShareViewerPrincipal } from './viewer-principal.ts';
/** The two write actions a reader may take on a Session. */
export type ReaderWriteAction = 'session/prompt' | 'session/cancel';
/** Share state this policy reads, all synchronous and held in memory by the share host. */
export interface ViewerAccessFacts {
    /**
     * Share state as the owner host holds it.
     * @param shareId - Share the reader's calls arrive through.
     * @returns The state, with `sessionMode` already narrowed to what may still be driven; undefined when
     *   this host publishes no such share.
     */
    readonly share: (shareId: string) => ForkShareState | undefined;
    /**
     * Whether the Relay authority this host holds still admits one reader.
     * @param shareId - Share the reader belongs to.
     * @param actor - Reader identity.
     * @returns True while admitted.
     */
    readonly viewerAdmitted: (shareId: string, actor: ShareRpcActor) => boolean;
    /**
     * Whether the share's source Session carries a usable confined access layer.
     * @param shareId - Share whose source Session is asked about.
     * @returns True only while the layer is installed and confined.
     */
    readonly sourceConfined: (shareId: string) => boolean;
    /** Forks this host created, and the per-child prompt decision. */
    readonly forks: Pick<ForkRegistry, 'find' | 'listForShare' | 'allowsPrompt'>;
    /**
     * Workspaces this host holds, when the composition mounts the Workspace registry.
     * @returns Workspaces in registry order.
     */
    readonly workspaces: () => readonly Pick<Workspace, 'id' | 'sessionIds'>[];
    /**
     * The endpoints plugins currently offer for share viewers, grouped by declaring plugin.
     *
     * Read on every binding computation and never captured, so a declaration and its disposal decide the
     * reader's next call. This is an offer, not a grant: the share's own selection decides what it exposes.
     * @returns The declared endpoints, empty when no plugin declares any.
     */
    readonly declaredCapabilities: () => readonly ViewerCapabilityGroup[];
}
/**
 * Every action this policy's own roles decide, derived from the endpoint classification table.
 *
 * A plugin capability adds only endpoints the share policy does not decide itself: the read, fork, and
 * write roles keep their conditions and their modes for the actions they list, so offering and selecting
 * `session/prompt`, `session/cancel`, `session/fork`, or `fileUploads/upload` cannot replace the decision
 * the policy's own roles make. This is not a whitelist: an endpoint no role lists stays refused until a
 * plugin offers it and the owner selects it.
 *
 * The table is the single source: `reader-*` entries plus the forwarded events reproduce exactly the
 * actions the roles list. The load-time assertion below fails loudly if either side drifts, so a
 * classification edit can never silently change what a reader may call.
 */
export declare const POLICY_DECIDED_ACTIONS: ReadonlySet<RemoteAccessAction>;
/**
 * The refusal code of one reader prompt or cancel, by the rules the policy's write bindings apply.
 *
 * The viewer reader reads this before it prepares or dispatches a write, so that a refusal keeps the
 * page's code and a refused write prepares nothing. The Gateway decides every dispatched call again from
 * the policy.
 * @param facts - Share state the policy reads.
 * @param viewer - The reader.
 * @param action - The write action.
 * @param sessionId - Session the write names.
 * @param concrete - True for a concrete call, false for the snapshot answer.
 * @returns The refusal code, or undefined when the policy allows the write.
 */
export declare function readerWriteRefusal(facts: ViewerAccessFacts, viewer: ShareViewerPrincipal, action: ReaderWriteAction, sessionId: string, concrete: boolean): string | undefined;
/**
 * Build the `share-viewer` policy for this host's shares.
 * @param facts - Share state the rules read.
 * @returns The policy the share host registers for the `share-viewer` principal kind.
 */
export declare function createViewerAccessPolicy(facts: ViewerAccessFacts): RemoteAccessPolicy<ShareViewerPrincipal>;
//# sourceMappingURL=viewer-access-policy.d.ts.map