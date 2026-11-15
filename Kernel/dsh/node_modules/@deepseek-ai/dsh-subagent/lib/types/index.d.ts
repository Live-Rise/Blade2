/**
 * Service Definition for the subagent capability seam (`ctx.subagents`): a named-provider registry plus a
 * capability-validating asynchronous start API. Providers establish a
 * child before returning its run, so fulfillment is the single publication and
 * ownership-transfer boundary.
 *
 * Multiple providers coexist: each registers under a unique name and callers
 * select one by name.
 *
 * This package owns the Service Definition role of the capability seam. Service Providers
 * (`@deepseek-ai/dsh-subagent-spawn-in-process`, `-fork`, `-acp`) and the model-facing
 * consumer (`@deepseek-ai/dsh-tool-subagent`) are separate packages.
 *
 * `startActivation` establishes every child under the activation manager, which
 * owns execution, result delivery, and resource release. Local providers seed
 * resumable Agents; external providers return a single execution handle.
 * `sendMessage` steers between local Agents in one subagent tree without exposing
 * whether a child is resident. Discovery combines real child Sessions with
 * parent-owned records for external executions.
 * Direct discovery reads parent catalogs; descendants recursively read reachable
 * child catalogs. External entries are leaves without local Sessions.
 *
 * Same-process providers are trusted typed collaborators. Requests, provider
 * descriptors, results, and lifecycle payloads are borrowed immutable values;
 * serialization and hostile-input validation belong at real process, worker,
 * persistence, and model boundaries.
 *
 * @module @deepseek-ai/dsh-subagent
 */
import type { Volatile } from '@deepseek-ai/cordis';
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Scoped } from '@deepseek-ai/dsh-scope';
import type { ContentBlock, MessageId } from '@deepseek-ai/dsh-llm';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { SessionId } from '@deepseek-ai/dsh-session';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { SubagentInterruptReceipt, SubagentPromptReceipt, SubagentPromptRequest } from './control-types.ts';
import type { SubagentActivation, SubagentActivationSpec, SubagentInterruptAuthority, SubagentMemberNames, SubagentProvider, SubagentRunEndInfo, SubagentRunInfo, SubagentSendMessageOptions } from './types.ts';
import type { SubagentDescendantListEntry } from './list-children.ts';
export type {} from './catalog.ts';
export type {} from './projection.ts';
export * from './out-of-process.ts';
export { AssistantOutputFold, finalAssistantOutput } from './assistant-output.ts';
export { SubagentRunId } from './types.ts';
export { ROOT_AGENT_NAME } from './names.ts';
export type { ContinuableCreateRequest, ContinuableCreateSpec, SubagentCapabilities, SubagentActivation, SubagentActivationSpec, SubagentInterruptAuthority, SubagentMemberNames, SubagentProvider, SubagentResult, SubagentRun, SubagentSendMessageOptions, SubagentStartRequest, ResolvedSubagentStartRequest, SubagentStopReason, SubagentStopReasonMap, } from './types.ts';
export { foldSubagentDescriptor, snapshotSubagentDescriptor, SUBAGENT_DESCRIPTOR_VERSION, } from './descriptor.ts';
export type { ContinuableSubagentDescriptorData, ContinuableSubagentDescriptorInput, OneShotSubagentDescriptorData, SubagentDescriptorData, } from './descriptor.ts';
export { STRUCTURED_OUTPUT_TOOL } from './structured.ts';
export { SubagentError } from './error.ts';
export { assertSubagentMaxDepth, delegationDepthOf } from './depth.ts';
export { appendDelegatedPolicyOverrides, applyChildComposition, captureDelegatedPolicyOverrides, childSessionMeta, parentAgentOptionsForDelegation, resolveChildAgentOptions, resolveChildDepth, SubagentDepthError, } from './child-agent.ts';
export type { ChildComposition, DelegatedPolicyOverrides } from './child-agent.ts';
export type { AgentMessageSource, SubagentSettledMessageSource } from './continuation-messages.ts';
export type * from './control-types.ts';
export type { SubagentDescendantListEntry } from './list-children.ts';
export type { SubagentRunEndInfo, SubagentRunInfo } from './types.ts';
export type { SubagentCatalogEntry, SubagentIdentityProjection, SubagentTimingProjection } from './projection-types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        subagents: SubagentRuntime;
    }
    interface Events {
        /**
         * A provider became resolvable in the registry.
         * @param provider - the registered provider.
         * @mode emit
         */
        'subagent/provider-added'(provider: SubagentProvider): void;
        /**
         * A provider left the registry. Accepted runs remain holder-owned.
         * @param name - the provider name that no longer resolves.
         * @mode emit
         */
        'subagent/provider-removed'(name: string): void;
        /**
         * A provider established a published child. For in-process providers,
         * `ctx.agents.get(info.id)` resolves during this notification.
         * Scope-filtered dispatch keys the carrier by the subagent tree root, so a
         * root-scoped listener observes all subagent tree activations. Paired with
         * `subagent/end`.
         * @param info - provider, child identity, and durable direct creator `parentSessionId`.
         * @mode emit
         */
        'subagent/start'(this: Scoped<SubagentRuntime>, info: SubagentRunInfo): void;
        /**
         * A published child settled. Scope-filtered dispatch uses the same subagent tree-root
         * carrier as `subagent/start`, so the lifecycle pair reaches the
         * same scoped audience.
         * @param info - run identity, durable direct creator `parentSessionId`, and terminal outcome.
         * @mode emit
         */
        'subagent/end'(this: Scoped<SubagentRuntime>, info: SubagentRunEndInfo): void;
    }
}
/** Host configuration for continuable subagent capacity. */
export interface Config {
    /** Maximum live children sharing uninterrupted continuable parent links; defaults to 8. */
    maxActiveSubagents: Volatile<number>;
    /** Default delegation depth for tools without an explicit limit; defaults to 1. */
    maxDepth: Volatile<number>;
}
/** Named provider registry with managed activations, durable discovery, and local child messaging. */
export declare class SubagentRuntime extends TypertRemoteService {
    private config;
    static Config: z<Schemastery.ObjectS<NoInfer<{
        maxDepth: z<number, number, "volatile-defined">;
        maxActiveSubagents: z<number, number, "volatile-defined">;
    }>>, Schemastery.ObjectT<NoInfer<{
        maxDepth: z<number, number, "volatile-defined">;
        maxActiveSubagents: z<number, number, "volatile-defined">;
    }>>, "plain">;
    static inject: string[];
    private providers;
    private manager;
    /**
     * The contained lifecycle-edge publisher. Built here because scoped dispatch
     * keys its carrier by this exact service instance, whose own context filter
     * composes into the carrier.
     */
    private readonly emitLifecycle;
    constructor(ctx: Context, config: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Resolve a delegation tool's depth policy against the current user setting.
     * @param configured - Explicit tool limit, or provider-managed for external delegation.
     * @returns The numeric limit, or undefined when the provider owns depth enforcement.
     */
    resolveMaxDepth(configured?: number | 'provider-managed'): number | undefined;
    /**
     * Start a local child under its reserved identity.
     * @param spec - local task, reserved child id, and result recipient.
     * @returns activation with its accepted initial message id.
     */
    startActivation(spec: SubagentActivationSpec & {
        readonly childId: SessionId;
    }): Promise<SubagentActivation & {
        readonly messageId: MessageId;
    }>;
    /**
     * Start a local or external child.
     * @param spec - task, backend, and result recipient.
     * @returns activation with a message id only for local children.
     */
    startActivation(spec: SubagentActivationSpec): Promise<SubagentActivation>;
    /**
     * Resolve the root shared by an Agent and its other agents in its subagent tree.
     * @param caller - exact live Agent whose subagent ancestry selects the tree.
     * @returns the live root; ordinary Session forks lead independent trees.
     * @throws when the caller or its runtime owner is unavailable.
     */
    rootOf(caller: Agent): Agent;
    /**
     * Report the model-facing names of a live tree member and its direct creator.
     * The root is `lead`; a descendant is the slash-separated path of names from
     * the root, or its Session id when it or a creator below the root has no name.
     * @param agent - exact live root or resident tree member.
     * @returns the member's name and, for a descendant, its creator's name.
     * @throws when the caller or its runtime owner is unavailable.
     */
    namesOf(agent: Agent): SubagentMemberNames;
    /**
     * Resolve a model-supplied agent reference within the caller's tree. `lead`
     * selects the root and a slash-separated path is looked up through the
     * catalogs along it. Any other text that is not a name in the tree is
     * returned as a Session id, because a single token can be one; a
     * slash-separated address never is, so an unmatched path fails instead of
     * reaching a member lookup as an unknown id. Resolution grants no authority:
     * messaging, interruption, and assignment authorize the result.
     * @param caller - exact live Agent whose tree root anchors the lookup.
     * @param name - `lead`, a full name from the root, or a Session id.
     * @param signal - cancellation forwarded to each catalog read.
     * @returns the referenced Session id.
     * @throws {@link SubagentError} `AGENT_NAME_NOT_FOUND` when a path names no member;
     *   when the caller's root is unavailable; or when a catalog along the path cannot be read.
     */
    resolveName(caller: Agent, name: string, signal: AbortSignal): Promise<SessionId>;
    /**
     * Join progressing subagent tree work for its root. Idle members with parked input
     * stay resident without delaying host completion. Ordinary members return false.
     * @param root - exact live subagent tree root whose member work is awaited.
     * @returns whether work was joined; hosts recheck root idle after true.
     */
    waitForTreeWork(root: Agent): Promise<boolean>;
    /**
     * Steer one model-authored message to another member of the same subagent tree.
     * A running target receives it at its next step; an idle target starts a turn.
     * A cold target resumes under its runtime subagent tree root without restoring
     * its creation ancestors. Sender identity is rechecked before inbox acceptance; cancellation
     * stops only pre-acceptance work and never retracts an accepted message.
     * @param sender - exact live root or resident continuable Agent originating the message.
     * @param targetId - another root or continuable member's durable Session id.
     * @param content - model-authored content to deliver.
     * @param options - caller cancellation before inbox acceptance.
     * @returns the accepted message's inbox id.
     * @throws when the target is outside the tree, cannot resume, or rejects input.
     */
    sendMessage(sender: Agent, targetId: SessionId, content: ContentBlock[], options: SubagentSendMessageOptions): Promise<MessageId>;
    /**
     * Interrupt one live child's current execution under a trusted user
     * operation or an exact live ancestor Agent. Fire-and-return: the cancel
     * signal is issued before this returns, but the target may keep running
     * until it observes the signal. Unclaimed pending inbox work, the Activation,
     * and published descendants are preserved; claimed work is not requeued.
     * Once the interrupted Agent is idle, a waking send resumes the parked FIFO
     * queue. External backends stop their single execution. An absent target
     * is an accepted no-op, as is a manager-less composition, which cannot own a
     * live Activation.
     * @param targetSessionId - the durable child session id to interrupt.
     * @param authority - a trusted user operation or exact live ancestor Agent.
     * @throws {SubagentError} `UNAUTHORIZED` when the authority does not own the
     *   live target.
     */
    interrupt(targetSessionId: SessionId, authority: SubagentInterruptAuthority): void;
    /**
     * Close admission and stop all instances in the selected subagent trees, then await
     * pending startup and cleanup. Ordinary members are ignored. The cutoff lasts
     * until each exact root leaves the registry; unrelated subagent trees remain live.
     * @param parents - exact live subagent tree roots entering teardown.
     * @returns once all admitted startup and member cleanup have settled.
     * @throws an aggregate error after all branches settle when any failed.
     */
    drainDescendants(parents: readonly Agent[]): Promise<void>;
    /**
     * Recursively list reachable parent catalogs in stable pre-order, preserving
     * each catalog's event order. Each row carries its catalog parent and depth;
     * external children are leaves; one-shot and unknown-mode children remain
     * traversal nodes. Unknown modes
     * produce unsupported diagnostics. Unreadable child catalogs produce corrupt
     * or unavailable diagnostics and stop only that branch. Root read failures,
     * missing services or projections, and cancellation reject the whole listing.
     * Each catalog is observed once and released before the next read. No Agent
     * is loaded or resumed; Sessions absent from reachable catalogs are omitted.
     * @param rootSessionId - session whose catalog starts descendant discovery.
     * @param signal - cancellation forwarded to and checked around each catalog read.
     * @returns children and branch diagnostics in parent-catalog pre-order.
     * @throws {@link SubagentError} when listing dependencies are unavailable or the caller cancels.
     * @throws SessionQueryError when the root catalog cannot be read.
     */
    listDescendants(rootSessionId: SessionId, signal?: AbortSignal): Promise<SubagentDescendantListEntry[]>;
    /**
     * Deliver one browser-authored message to a continuable child through its
     * live subagent tree root, retaining the caller-minted request identity and
     * validated browser zone on the accepted message. Success identifies the
     * message the child's inbox accepted; later execution is independent of this
     * call. Queue delivery targets a later turn; steer delivery targets the
     * nearest step and retains the Agent loop's best-effort fallback semantics.
     * Image parts are admitted and persisted through the attachment store
     * before delivery, and the child's model must accept image input.
     * Cold resume at capacity rejects with `subagent/delivery-unavailable`.
     * Access is checked on the target Session before attachment admission or restoration.
     * @param request - target id, delivery, minted identity, content, and optional browser zone.
     * @param signal - carrier cancellation, owning the call until inbox acceptance.
     * @returns the accepted message's inbox identity.
     * @throws {RemoteError} `gateway/bad-request`, `subagent/attachment-invalid`,
     *   `subagent/invalid-time-zone`,
     *   `subagent/not-resumable`, `subagent/unauthorized`,
     *   `subagent/delivery-unavailable`, `gateway/cancelled`, or `gateway/internal`.
     */
    prompt(request: SubagentPromptRequest, signal: AbortSignal): Promise<SubagentPromptReceipt>;
    /**
     * Remote face of {@link interrupt}, authorized on the target Session by the Gateway.
     * No catalog, history, persistence, or parent Agent lookup runs.
     * Absent, idle, and already-completed targets are accepted no-ops there.
     * @param childSessionId - durable child session id to interrupt.
     * @returns acknowledgement that the cancel signal was admitted, not that the target is quiescent.
     * @throws {RemoteError} `gateway/bad-request` for an empty id,
     *   otherwise `gateway/internal`.
     */
    interruptByUser(childSessionId: SessionId): SubagentInterruptReceipt;
    /**
     * Register a provider under its name. Registration is effect-scoped and HMR
     * safe; removing a provider blocks new starts but does not revoke runs that
     * were already returned to their holders. Providers without either execution
     * method are rejected with UNSUPPORTED_CAPABILITY before registration.
     * @param provider - the trusted provider implementation.
     * @returns the exact Cordis effect disposer.
     */
    registerProvider(provider: SubagentProvider): () => void;
    /**
     * Look up a provider by name.
     * @param name - the provider name.
     * @returns the provider, or undefined when absent.
     */
    getProvider(name: string): SubagentProvider | undefined;
    /** A pending start must use the exact provider whose capabilities were checked. */
    private assertProviderCurrent;
    /** Look up a provider for dispatch or fail loud. */
    private expectProvider;
    /** Resolve the subagent manager or fail loud. */
    private requireManager;
    /** Reject the first requested capability that the provider lacks. */
    private assertCapabilities;
}
export default SubagentRuntime;
//# sourceMappingURL=index.d.ts.map