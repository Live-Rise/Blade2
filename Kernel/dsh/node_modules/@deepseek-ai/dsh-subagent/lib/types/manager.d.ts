/**
 * Owns subagent startup, message admission, cold resume, and execution lifetimes.
 * One child lock orders input acceptance and closure; one activation map owns
 * capacity, runtime ownership, and resource release across all providers.
 *
 * @module @deepseek-ai/dsh-subagent/manager
 */
import { type Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { ContentBlock, MessageId, MessageSource } from '@deepseek-ai/dsh-llm';
import type { Session, SessionId } from '@deepseek-ai/dsh-session';
import type { LifecycleEmitter } from './lifecycle.ts';
import type { SubagentCatalogEntry, SubagentIdentityProjection } from './projection-types.ts';
import type { ContinuableCreateRequest, ContinuableCreateSpec, SubagentInterruptAuthority, SubagentActivationSpec, SubagentActivation, SubagentMemberNames, SubagentRun, SubagentSendMessageOptions, ResolvedSubagentStartRequest } from './types.ts';
import type { SubagentDelivery } from './control-types.ts';
/** Inputs shared by model steering and human prompt delivery. */
type ChildDeliveryOptions = {
    readonly delivery: SubagentDelivery;
    readonly source: MessageSource;
    readonly signal: AbortSignal;
    /** Recheck the original model caller at final inbox admission. */
    readonly authorize?: () => void;
    /** Record successful inbox admission before another asynchronous operation can intervene. */
    readonly onAccepted?: () => void;
};
/** Package-private hooks supplied by the owning service. */
interface SubagentHost {
    /** Read the current descriptor identity through the service-owned projection. */
    identityOf(session: Session): SubagentIdentityProjection | undefined;
    /** Read a live Session's direct-child catalog through the service-owned projection. */
    catalogOf(session: Session): readonly SubagentCatalogEntry[];
    /** Publish lifecycle events through the owning service's scoped dispatcher. */
    readonly emitLifecycle: LifecycleEmitter;
}
/** Own subagent requests and the resources of every live activation. */
export declare class SubagentManager {
    private readonly ctx;
    private readonly host;
    private readonly maxActiveSubagents;
    /** Child session id → its live Activation. Process-local, never durable. */
    private readonly resident;
    /** Root identities retain their pool across child settlement without retaining dead roots. */
    private readonly rootPools;
    /** Materializations admitted before drain, tracked through publication or rollback. */
    private readonly materializations;
    /** Per-child serializer shared by delivery, release, and disposal. */
    private readonly locks;
    /** Structural Cordis owner of every Activation handle. */
    private readonly ownerCtx;
    /** Exact subagent tree roots whose host teardown has closed admission. */
    private readonly closingRoots;
    /** Sibling names held by creations that have not committed their catalog entry. */
    private readonly names;
    private draining;
    /**
     * Build one manager inside the service's Agent-injected context.
     * @param ctx - context providing Agents, Sessions, and teardown ownership.
     * @param host - descriptor reads and lifecycle publication owned by the service.
     * @param maxActiveSubagents - the current configured child capacity.
     */
    constructor(ctx: Context, host: SubagentHost, maxActiveSubagents: () => number);
    /**
     * Interrupt one live continuable child's current turn under the supplied authority.
     * @param targetSessionId - the durable child session id to interrupt.
     * @param authority - a trusted user operation or exact live ancestor Agent.
     */
    interrupt(targetSessionId: SessionId, authority: SubagentInterruptAuthority): void;
    /**
     * Close admission, await every already-admitted materialization through
     * publication or rollback, then dispose every agent in the subagent tree.
     */
    drain(): Promise<void>;
    /**
     * Join progressing subagent tree work for its root, leaving idle parked inboxes resident.
     * @param root - exact live subagent tree root; ordinary members return false.
     * @returns whether work was joined and the host must recheck root completion.
     */
    waitForTreeWork(root: Agent): Promise<boolean>;
    /**
     * Stop every member of the selected subagent trees; ordinary members do not close a subagent tree.
     * @param parents - exact live subagent tree roots whose instances must stop.
     */
    drainDescendants(parents: readonly Agent[]): Promise<void>;
    /**
     * Start one continuable background child and resolve at initial inbox acceptance.
     * Every earlier failure disposes any created handle and rolls back Activation
     * and its capacity slot without returning either id.
     * @param spec - provider, delegation request, and caller cancellation.
     * @param prepare - creation contribution bound to the checked provider.
     * @returns the durable child id and accepted initial prompt message id.
     */
    startLocal(spec: SubagentActivationSpec, prepare: (request: ContinuableCreateRequest) => Promise<ContinuableCreateSpec>): Promise<SubagentActivation & {
        messageId: MessageId;
    }>;
    /** Create one local child under a name reserved before any await. */
    private startLocalNamed;
    /**
     * Start an external backend under activation ownership.
     * @param spec - task, provider, cancellation, and delivery policy.
     * @param start - execution bound to the checked provider.
     * @returns the accepted activation and its result/disposal capabilities.
     */
    startExternal(spec: SubagentActivationSpec, start: (request: ResolvedSubagentStartRequest) => Promise<SubagentRun>): Promise<SubagentActivation>;
    /** Start one external execution, cataloging parent-delivered runs under their reserved name. */
    private startExternalNamed;
    /** Capture the explicit or inherited directory while the parent still owns startup. */
    private resolveDirectory;
    /**
     * Resolve the live subagent tree root from runtime ownership, independent of creator residency.
     * @param caller - exact live Agent selecting the subagent tree.
     * @returns the live subagent tree root.
     * @throws when the caller has no active runtime owner.
     */
    rootOf(caller: Agent): Agent;
    /** Capture creation ancestry separately from the root that owns the new instance. */
    private ancestryForChild;
    /** Claim a sibling name against the creator's committed catalog and in-flight siblings. */
    private reserveName;
    /** Path of an exact live tree member: `''` for the root, undefined for unnamed history. */
    private pathOf;
    /**
     * Report the model-facing names of a live member and its direct creator.
     * @param agent - exact live root or resident tree member.
     * @returns `lead` for the root, otherwise the member's and creator's names or Session ids.
     * @throws when the caller has no active runtime owner.
     */
    namesOf(agent: Agent): SubagentMemberNames;
    /**
     * Deliver to another agent in the subagent tree, restoring only the target when absent.
     * @param sender - exact live root or resident continuable Agent.
     * @param targetId - another member's durable Session id.
     * @param content - model-authored message blocks.
     * @param options - cancellation before inbox acceptance.
     * @returns the accepted inbox message id.
     */
    sendMessage(sender: Agent, targetId: SessionId, content: ContentBlock[], options: SubagentSendMessageOptions): Promise<MessageId>;
    /** Resolve the live owner through verified creation edges without activating ancestors. */
    private resolveCreationAncestry;
    /**
     * Deliver one host-authored prompt through the target's live subagent tree root.
     * Remote callers must be authorized for the target Session before invoking this operation.
     * @param childId - durable continuable subagent session id.
     * @param content - model-visible prompt blocks.
     * @param options - inbox destination, durable attribution, and cancellation before acceptance.
     * @returns the accepted durable message id.
     */
    deliverPrompt(childId: SessionId, content: ContentBlock[], options: Pick<ChildDeliveryOptions, 'delivery' | 'source' | 'signal'>): Promise<MessageId>;
    /** Admit local input synchronously before any teardown can begin. */
    private deliver;
    /**
     * Reject one child identity already owned by a live Agent or Session.
     * @param childId - proposed durable child session id.
     */
    private assertChildIdAvailable;
    /**
     * Apply activation admission to a resident parent before delivering input.
     * @param parent - exact live Agent receiving the message.
     * @param message - durable user message to deliver.
     * @param delivery - receiving inbox destination.
     */
    private sendWaking;
    /**
     * Reject new work from closing members or subagent trees.
     * @param agent - exact live caller whose instance and subagent tree must admit work.
     */
    private assertAdmitting;
    /**
     * Create or resume one child Agent and publish its Activation.
     * @param inputs - reconstruction and admission inputs for the residency epoch.
     * @returns the published process-local Activation.
     */
    private materialize;
    /** Keep subagent tree shutdown and host waits joined to preparation, publication, and rollback. */
    private trackStartup;
    /**
     * Stop and release one Activation through its memoized close transaction.
     * @param activation - exact residency epoch to close.
     * @returns the shared close transaction.
     */
    private dispose;
    /** Close admission synchronously and share one release, including startup rollback. */
    private close;
    /** Dispose independent roots and report every branch failure after all settle. */
    private disposeRoots;
    /** Return the subagent tree teardown that closed admission for this exact Agent. */
    private closingTeardownFor;
    /** Share one capacity pool across every instance in a subagent tree. */
    private rootPool;
    /** Perform one tracked materialization through publication or rollback. */
    private materializeTracked;
    /** Identity shared by one residency epoch's start/end pair, including cold resumes. */
    private runInfo;
    /** Record acceptance and begin settlement observation before returning the receipt. */
    private accept;
    /** Release an Activation whose start edge was not published. */
    private rollbackUnpublished;
    /** Let a settlement watcher re-check residency after relevant state changes. */
    private wake;
    /** Follow one Activation to natural settlement. */
    private watchLocalSettlement;
    /** Report disposal failure once after the natural settlement observer finishes. */
    private reportTeardownFailure;
    /** Check pending input before closing admission. */
    private settlementState;
    /** Stop and release one instance without affecting its creation descendants. */
    private finishDisposal;
    /** Deliver the captured execution result to the durable direct parent after cleanup. */
    private notifySettlement;
    /** Request a best-effort final session flush before closing natural-settlement admission. */
    private flushFinalState;
    /** Return capabilities bound to an exact activation instead of a future same-id instance. */
    private receipt;
    /** Serialize target restoration and inbox admission; closure is awaited outside the lock. */
    private deliverToMember;
    /** Read continuation history, preserving cancellation and reporting unavailable storage. */
    private observeContinuation;
    /** Reconstruct only the target under its subagent tree root; the caller owns acceptance or rollback. */
    private restoreTarget;
    /** Admit a materialized child, commit its creation fact, and release it on failure. */
    private submitMaterialized;
    /** Build and submit one message across the final synchronous admission cutoff. */
    private submitAdmitted;
    /** Refuse image content for a child whose fixed model accepts text only. */
    private assertImageCapable;
    /** Resolve the persistence service continuable children require, or fail loud. */
    private requirePersistence;
    /** Resolve the Session query service used for cold child observations. */
    private requireSessionQuery;
}
export default SubagentManager;
//# sourceMappingURL=manager.d.ts.map