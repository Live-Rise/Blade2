/**
 * Owns subagent startup, message admission, cold resume, and execution lifetimes.
 * One child lock orders input acceptance and closure; one activation map owns
 * capacity, runtime ownership, and resource release across all providers.
 *
 * @module @deepseek-ai/dsh-subagent/manager
 */
var __addDisposableResource = (this && this.__addDisposableResource) || function (env, value, async) {
    if (value !== null && value !== void 0) {
        if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
        var dispose, inner;
        if (async) {
            if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
            dispose = value[Symbol.asyncDispose];
        }
        if (dispose === void 0) {
            if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
            dispose = value[Symbol.dispose];
            if (async) inner = dispose;
        }
        if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
        if (inner) dispose = function() { try { inner.call(this); } catch (e) { return Promise.reject(e); } };
        env.stack.push({ value: value, dispose: dispose, async: async });
    }
    else if (async) {
        env.stack.push({ async: true });
    }
    return value;
};
var __disposeResources = (this && this.__disposeResources) || (function (SuppressedError) {
    return function (env) {
        function fail(e) {
            env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
            env.hasError = true;
        }
        var r, s = 0;
        function next() {
            while (r = env.stack.pop()) {
                try {
                    if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
                    if (r.dispose) {
                        var result = r.dispose.call(r.value);
                        if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) { fail(e); return next(); });
                    }
                    else s |= 1;
                }
                catch (e) {
                    fail(e);
                }
            }
            if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
            if (env.hasError) throw env.error;
        }
        return next();
    };
})(typeof SuppressedError === "function" ? SuppressedError : function (error, suppressed, message) {
    var e = new Error(message);
    return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
import { randomUUID } from 'node:crypto';
import { isAbsolute, resolve } from 'node:path';
import { brandString } from '@deepseek-ai/dsh-brand';
import { ReasoningEffortId, contentHasImage, createUserMessage, errorChain } from '@deepseek-ai/dsh-llm';
import { SessionLogOffset } from '@deepseek-ai/dsh-session';
import { appendDelegatedPolicyOverrides, applyChildComposition, captureDelegatedPolicyOverrides, childSessionMeta, resolveChildAgentOptions, resolveChildDepth, } from "./child-agent.js";
import { createAgentMessage, createSettlementMessage, withResultDeliveryGuidance, } from "./continuation-messages.js";
import { installSubagentArchiveAdmission } from "./archive-admission.js";
import { foldSubagentDescriptor, snapshotSubagentDescriptor } from "./descriptor.js";
import { catalogNames, establishCatalogChild, establishExternalCatalogChild } from "./catalog.js";
import { SubagentError } from "./error.js";
import { ROOT_AGENT_NAME, SiblingNameReservations, agentAddress, joinAgentPath, parentAgentPath } from "./names.js";
import { SubagentRunId } from "./types.js";
import { attachStructuredRuntime } from "./structured.js";
import { ActivationPool, ChildLock, activationAgent, captureLocalResult, observeLocalFailure, requireLocalActivation } from "./activation.js";
/** Render only the safe outer message from execution or teardown failures. */
function failureMessage(error) {
    return error instanceof Error ? error.message : 'unknown teardown failure';
}
/** Own subagent requests and the resources of every live activation. */
export class SubagentManager {
    ctx;
    host;
    maxActiveSubagents;
    /** Child session id → its live Activation. Process-local, never durable. */
    resident = new Map();
    /** Root identities retain their pool across child settlement without retaining dead roots. */
    rootPools = new WeakMap();
    /** Materializations admitted before drain, tracked through publication or rollback. */
    materializations = new Set();
    /** Per-child serializer shared by delivery, release, and disposal. */
    locks = new ChildLock();
    /** Structural Cordis owner of every Activation handle. */
    ownerCtx;
    /** Exact subagent tree roots whose host teardown has closed admission. */
    closingRoots = new Set();
    /** Sibling names held by creations that have not committed their catalog entry. */
    names = new SiblingNameReservations();
    draining = false;
    /**
     * Build one manager inside the service's Agent-injected context.
     * @param ctx - context providing Agents, Sessions, and teardown ownership.
     * @param host - descriptor reads and lifecycle publication owned by the service.
     * @param maxActiveSubagents - the current configured child capacity.
     */
    constructor(ctx, host, maxActiveSubagents) {
        this.ctx = ctx;
        this.host = host;
        this.maxActiveSubagents = maxActiveSubagents;
        // Drain managed instances before Cordis releases their owning scope.
        const scope = ctx.plugin(function activationOwner() { });
        this.ownerCtx = scope.ctx;
        installSubagentArchiveAdmission(ctx, (agent) => {
            const activation = this.resident.get(agent.id);
            return activation !== undefined && activationAgent(activation) === agent ? activation.ancestors : undefined;
        }, session => host.identityOf(session));
        ctx.on('agent/disposed', ({ agent }) => {
            this.closingRoots.delete(agent);
            const activation = this.resident.get(agent.id);
            if (activation?.kind === 'local' && activation.handle.agent === agent) {
                void this.close(activation, () => this.finishDisposal(activation, false)).catch((error) => {
                    this.reportTeardownFailure(activation, error);
                });
            }
        });
        ctx.effect(function* () {
            yield scope.dispose;
            yield () => this.drain();
        }.bind(this), 'subagents.manager()');
    }
    /**
     * Interrupt one live continuable child's current turn under the supplied authority.
     * @param targetSessionId - the durable child session id to interrupt.
     * @param authority - a trusted user operation or exact live ancestor Agent.
     */
    interrupt(targetSessionId, authority) {
        if (authority.kind === 'ancestor') {
            const caller = authority.agent;
            if (this.ctx.agents.get(caller.id) !== caller) {
                throw new SubagentError(`interrupting "${targetSessionId}" requires the exact live ancestor agent`, 'UNAUTHORIZED');
            }
            if (caller.id === targetSessionId) {
                throw new SubagentError(`agent "${caller.id}" cannot interrupt itself`, 'UNAUTHORIZED');
            }
        }
        const activation = this.resident.get(targetSessionId);
        if (activation === undefined)
            return;
        if (authority.kind === 'ancestor'
            && (this.rootOf(authority.agent) !== activation.root || !activation.ancestors.includes(authority.agent.id))) {
            throw new SubagentError(`subagent "${targetSessionId}" is not a live descendant of agent "${authority.agent.id}"`, 'UNAUTHORIZED');
        }
        // Disposal already stopped the target with a whole-Activation teardown;
        // a second cancel would be a redundant signal on a closing handle.
        if (activation.closing !== undefined)
            return;
        const kind = authority.kind === 'user' ? 'user' : 'parent';
        if (activation.kind === 'local') {
            activation.handle.agent.cancel({ kind }, { keepInbox: true });
        }
        else {
            activation.controller.abort({ kind });
            // The settlement observer reports teardown failures.
            void this.dispose(activation).catch(() => undefined);
        }
    }
    /**
     * Close admission, await every already-admitted materialization through
     * publication or rollback, then dispose every agent in the subagent tree.
     */
    async drain() {
        this.draining = true;
        for (const pending of this.materializations)
            pending.controller.abort();
        await Promise.all([...this.materializations].map(materialization => materialization.settled));
        await this.disposeRoots([...this.resident.values()], 'activation(s)');
    }
    /**
     * Join progressing subagent tree work for its root, leaving idle parked inboxes resident.
     * @param root - exact live subagent tree root; ordinary members return false.
     * @returns whether work was joined and the host must recheck root completion.
     */
    async waitForTreeWork(root) {
        if (this.rootOf(root) !== root)
            return false;
        let observed = false;
        while (true) {
            const pending = [...this.materializations].filter(item => item.root === root);
            const children = [...this.resident.values()].filter(item => item.root === root);
            observed ||= children.some(item => item.kind === 'local' && item.handle.agent.status !== 'idle');
            await Promise.all(children.flatMap(item => item.kind === 'local' ? [item.handle.agent.whenIdle()] : []));
            const waits = pending.map(item => item.settled);
            for (const item of children) {
                if (item.kind === 'external' || item.closing !== undefined) {
                    waits.push(item.result.promise.catch(() => undefined));
                    continue;
                }
                if (item.handle.agent.status !== 'idle') {
                    waits.push(item.handle.agent.whenIdle());
                    continue;
                }
                const inbox = item.handle.agent.inbox;
                if (item.failureAt === undefined && (inbox.nextTurn.length > 0 || inbox.nextStep.length > 0))
                    continue;
                waits.push(Promise.race([item.result.promise.catch(() => undefined), item.poke.promise]));
            }
            if (waits.length > 0) {
                observed = true;
                await Promise.all(waits);
                continue;
            }
            // A descendant can be published while whenIdle follows another child.
            if ([...this.materializations].some(item => item.root === root && !pending.includes(item))
                || [...this.resident.values()].some(item => item.root === root && !children.includes(item)))
                continue;
            return observed;
        }
    }
    /**
     * Stop every member of the selected subagent trees; ordinary members do not close a subagent tree.
     * @param parents - exact live subagent tree roots whose instances must stop.
     */
    async drainDescendants(parents) {
        const roots = new Set(parents.filter(root => this.ctx.agents.get(root.id) === root && this.rootOf(root) === root));
        for (const root of roots)
            this.closingRoots.add(root);
        const pending = [...this.materializations].filter(item => roots.has(item.root));
        for (const item of pending)
            item.controller.abort();
        const targets = [...this.resident.values()].filter(item => roots.has(item.root));
        const disposal = this.disposeRoots(targets, 'scoped activation(s)');
        const [disposed] = await Promise.allSettled([disposal, ...pending.map(item => item.settled)]);
        if (disposed.status === 'rejected')
            throw disposed.reason;
    }
    /**
     * Start one continuable background child and resolve at initial inbox acceptance.
     * Every earlier failure disposes any created handle and rolls back Activation
     * and its capacity slot without returning either id.
     * @param spec - provider, delegation request, and caller cancellation.
     * @param prepare - creation contribution bound to the checked provider.
     * @returns the durable child id and accepted initial prompt message id.
     */
    async startLocal(spec, prepare) {
        const parent = spec.parent;
        this.assertAdmitting(parent);
        const ancestry = this.ancestryForChild(parent);
        const reservation = this.reserveName(parent, spec.name);
        const path = joinAgentPath(this.pathOf(parent, ancestry.root), reservation.name);
        try {
            return await this.startLocalNamed(spec, prepare, ancestry, reservation, path);
        }
        finally {
            reservation.release();
        }
    }
    /** Create one local child under a name reserved before any await. */
    startLocalNamed(spec, prepare, ancestry, reservation, path) {
        const parent = spec.parent;
        return this.trackStartup(ancestry.root, spec.signal, async (signal) => {
            const persistence = spec.delivery === 'caller' ? this.ctx.get('sessionPersistence') : this.requirePersistence();
            const childId = spec.childId ?? brandString(randomUUID());
            this.assertChildIdAvailable(childId);
            const childDepth = resolveChildDepth(parent, spec.maxDepth);
            // Snapshot before any await: invalid descriptor JSON rejects the call
            // before a child exists, and the detached value is what reaches the log.
            const agentOptions = resolveChildAgentOptions(parent, spec.agentOptions, childDepth);
            const agentProvider = agentOptions.provider;
            const agentModel = agentOptions.model;
            const agentReasoningEffort = agentOptions.reasoningEffort;
            const descriptor = snapshotSubagentDescriptor({
                mode: 'continuable',
                provider: spec.provider,
                label: spec.label,
                ...agentProvider !== undefined ? { agentProvider } : {},
                ...agentModel !== undefined ? { agentModel } : {},
                ...agentReasoningEffort !== undefined ? { agentReasoningEffort } : {},
            });
            // Capture before the first await: a later parent switch belongs to the
            // parent's future, not to this child.
            const delegatedPolicies = captureDelegatedPolicyOverrides(parent);
            const cwd = await this.resolveDirectory(spec, signal);
            const prepared = await prepare({
                sessionId: childId,
                cwd,
                parent,
                signal,
            });
            signal.throwIfAborted();
            this.assertAdmitting(parent);
            const inheritedEventCount = SessionLogOffset(prepared.seed?.length ?? 0);
            const seed = prepared.seed;
            let established;
            const messageId = await this.locks.run(childId, async () => {
                signal.throwIfAborted();
                this.assertAdmitting(parent);
                this.assertChildIdAvailable(childId);
                if (spec.childId !== undefined) {
                    const persisted = await persistence?.stat(childId, { signal: signal });
                    signal.throwIfAborted();
                    this.assertAdmitting(parent);
                    this.assertChildIdAvailable(childId);
                    if (persisted !== undefined) {
                        throw new SubagentError(`subagent "${childId}" already exists`, 'DUPLICATE_CHILD');
                    }
                }
                const activation = await this.materialize({
                    kind: 'local',
                    childId,
                    provider: spec.provider,
                    ...ancestry,
                    path,
                    create: {
                        parent,
                        cwd,
                        seed,
                        meta: childSessionMeta(parent, childDepth, prepared.seed !== undefined, spec.agentPreset),
                        inheritedEventCount,
                        delegatedPolicies,
                        descriptor,
                    },
                    agentOptions,
                    composition: { agentPreset: spec.agentPreset },
                    signal,
                    delivery: spec.delivery,
                    outputSchema: spec.outputSchema,
                });
                established = activation;
                const child = requireLocalActivation(activation).handle.agent;
                const childHeader = child.session.header;
                return await this.submitMaterialized(activation, spec.delivery !== 'caller'
                    ? withResultDeliveryGuidance(spec.prompt)
                    : spec.prompt, { source: { kind: 'user' }, signal, delivery: 'queue', authorize: () => { this.assertAdmitting(parent); } }, ancestry.root, () => {
                    this.assertAdmitting(parent);
                    establishCatalogChild(parent.session, childHeader, descriptor, reservation.name);
                });
            });
            return { ...this.receipt(established), childId, messageId };
        });
    }
    /**
     * Start an external backend under activation ownership.
     * @param spec - task, provider, cancellation, and delivery policy.
     * @param start - execution bound to the checked provider.
     * @returns the accepted activation and its result/disposal capabilities.
     */
    async startExternal(spec, start) {
        const { delivery, name: requestedName, parent } = spec;
        this.assertAdmitting(parent);
        const ancestry = this.ancestryForChild(parent);
        if (delivery === 'caller' && requestedName !== undefined) {
            throw new SubagentError('agent names require a cataloged child; caller-delivered external runs are not cataloged', 'UNSUPPORTED_CAPABILITY');
        }
        // Only parent-delivered external runs enter the catalog, so only they hold a name.
        const reservation = delivery === 'parent' ? this.reserveName(parent, requestedName) : undefined;
        const path = joinAgentPath(this.pathOf(parent, ancestry.root), reservation?.name);
        try {
            return await this.startExternalNamed(spec, start, ancestry, reservation, path);
        }
        finally {
            reservation?.release();
        }
    }
    /** Start one external execution, cataloging parent-delivered runs under their reserved name. */
    startExternalNamed(spec, start, ancestry, reservation, path) {
        const { provider, delivery, childId: _childId, name: _name, ...request } = spec;
        const parent = request.parent;
        return this.trackStartup(ancestry.root, spec.signal, async (signal) => {
            let activation;
            try {
                const cwd = await this.resolveDirectory(spec, signal);
                activation = await this.materialize({
                    kind: 'external',
                    provider,
                    ...ancestry,
                    path,
                    signal,
                    delivery,
                    start: signal => start({
                        ...request, cwd, signal,
                    }),
                });
                this.assertAdmitting(parent);
                signal.throwIfAborted();
                if (reservation !== undefined) {
                    establishExternalCatalogChild(parent.session, activation.childId, spec.label, reservation.name);
                }
                this.accept(activation);
                return this.receipt(activation);
            }
            catch (error) {
                if (activation !== undefined) {
                    try {
                        await this.dispose(activation);
                    }
                    catch (cleanupError) {
                        this.ctx.logger.warn(`subagent "${activation.childId}" admission rollback failed: ${failureMessage(cleanupError)}`);
                    }
                }
                throw error;
            }
        });
    }
    /** Capture the explicit or inherited directory while the parent still owns startup. */
    async resolveDirectory(request, signal) {
        const cwd = request.cwd !== undefined && isAbsolute(request.cwd)
            ? request.cwd
            : resolve(await this.ctx.workingDirectory.ensure(request.parent, signal), request.cwd ?? '.');
        signal.throwIfAborted();
        return cwd;
    }
    /**
     * Resolve the live subagent tree root from runtime ownership, independent of creator residency.
     * @param caller - exact live Agent selecting the subagent tree.
     * @returns the live subagent tree root.
     * @throws when the caller has no active runtime owner.
     */
    rootOf(caller) {
        if (this.ctx.agents.get(caller.id) !== caller) {
            throw new SubagentError('subagent tree access requires the exact live caller', 'UNAUTHORIZED');
        }
        const identity = this.host.identityOf(caller.session);
        const delegated = caller.session.header.origin === 'subagent' || identity != null && caller.session.isOwnSeq(identity.seq);
        const activation = this.resident.get(caller.id);
        if (activation !== undefined && activationAgent(activation) === caller
            && this.ctx.agents.get(activation.root.id) === activation.root)
            return activation.root;
        if (delegated || activation !== undefined) {
            throw new SubagentError('agent has no active runtime owner', 'UNAUTHORIZED');
        }
        return caller;
    }
    /** Capture creation ancestry separately from the root that owns the new instance. */
    ancestryForChild(creator) {
        const root = this.rootOf(creator);
        const ancestors = this.resident.get(creator.id)?.ancestors ?? [];
        return { root, ancestors: [...ancestors, creator.id] };
    }
    /** Claim a sibling name against the creator's committed catalog and in-flight siblings. */
    reserveName(parent, requested) {
        return this.names.reserve(parent, catalogNames(this.host.catalogOf(parent.session)), requested);
    }
    /** Path of an exact live tree member: `''` for the root, undefined for unnamed history. */
    pathOf(member, root) {
        return member === root ? '' : this.resident.get(member.id)?.path;
    }
    /**
     * Report the model-facing names of a live member and its direct creator.
     * @param agent - exact live root or resident tree member.
     * @returns `lead` for the root, otherwise the member's and creator's names or Session ids.
     * @throws when the caller has no active runtime owner.
     */
    namesOf(agent) {
        const root = this.rootOf(agent);
        if (root === agent)
            return { self: ROOT_AGENT_NAME };
        // rootOf returns another root only for the caller's own resident Activation.
        const activation = this.resident.get(agent.id);
        const parentId = activation.ancestors.at(-1);
        const parentPath = activation.path === undefined
            ? parentId === root.id ? '' : this.resident.get(parentId)?.path
            : parentAgentPath(activation.path);
        return { self: agentAddress(activation.path, agent.id), parent: agentAddress(parentPath, parentId) };
    }
    /**
     * Deliver to another agent in the subagent tree, restoring only the target when absent.
     * @param sender - exact live root or resident continuable Agent.
     * @param targetId - another member's durable Session id.
     * @param content - model-authored message blocks.
     * @param options - cancellation before inbox acceptance.
     * @returns the accepted inbox message id.
     */
    async sendMessage(sender, targetId, content, options) {
        options.signal.throwIfAborted();
        const root = this.rootOf(sender);
        if (sender.id === targetId)
            throw new SubagentError('send_message requires another agent in the subagent tree', 'UNAUTHORIZED');
        this.assertAdmitting(sender);
        const message = createAgentMessage(sender, agentAddress(this.pathOf(sender, root), sender.id), content);
        const activation = this.resident.get(sender.id);
        return this.deliverToMember(root, targetId, [...message.content], {
            signal: options.signal, delivery: 'steer', source: message.source,
            authorize: () => { this.assertAdmitting(sender); },
            onAccepted: () => {
                if (activation?.kind === 'local' && activation.ancestors.at(-1) === targetId) {
                    activation.omitClosingText = true;
                }
            },
        });
    }
    /** Resolve the live owner through verified creation edges without activating ancestors. */
    async resolveCreationAncestry(source, signal, root) {
        const ancestors = [];
        const names = [];
        const seen = new Set([source.header.id]);
        let { header, projections, inheritedEventCount } = source;
        const pathFromRoot = () => names.reduce(joinAgentPath, '');
        while (true) {
            const env_1 = { stack: [], error: void 0, hasError: false };
            try {
                signal.throwIfAborted();
                const parentId = header.parentSession;
                const identity = projections?.values.subagent;
                if (parentId === undefined || identity == null || identity.seq < inheritedEventCount)
                    break;
                if (seen.has(parentId))
                    throw new SubagentError('cyclic subagent creation ancestry', 'UNAUTHORIZED');
                seen.add(parentId);
                const parent = __addDisposableResource(env_1, await this.observeContinuation(parentId, signal), false);
                const entry = parent.projections?.values.subagentCatalog?.find(entry => entry.id === header.id);
                if (entry === undefined) {
                    throw new SubagentError('subagent is absent from its creator catalog', 'UNAUTHORIZED');
                }
                ancestors.unshift(parentId);
                names.unshift('name' in entry ? entry.name : undefined);
                if (parentId === root?.id)
                    return { root, ancestors, path: pathFromRoot() };
                ({ header, projections, inheritedEventCount } = parent);
            }
            catch (e_1) {
                env_1.error = e_1;
                env_1.hasError = true;
            }
            finally {
                __disposeResources(env_1);
            }
        }
        if (root === undefined && ancestors.length > 0 && header.origin !== 'subagent') {
            const owner = this.ctx.agents.get(header.id);
            if (owner === undefined || this.rootOf(owner) !== owner) {
                throw new SubagentError('subagent tree root is not live', 'CONTINUATION_UNAVAILABLE');
            }
            return { root: owner, ancestors, path: pathFromRoot() };
        }
        throw new SubagentError(`agent "${source.header.id}" is not a member of this subagent tree`, 'UNAUTHORIZED');
    }
    /**
     * Deliver one host-authored prompt through the target's live subagent tree root.
     * Remote callers must be authorized for the target Session before invoking this operation.
     * @param childId - durable continuable subagent session id.
     * @param content - model-visible prompt blocks.
     * @param options - inbox destination, durable attribution, and cancellation before acceptance.
     * @returns the accepted durable message id.
     */
    async deliverPrompt(childId, content, options) {
        options.signal.throwIfAborted();
        let root = this.resident.get(childId)?.root;
        if (root === undefined) {
            const env_2 = { stack: [], error: void 0, hasError: false };
            try {
                const source = __addDisposableResource(env_2, await this.observeContinuation(childId, options.signal), false);
                ({ root } = await this.resolveCreationAncestry(source, options.signal));
            }
            catch (e_2) {
                env_2.error = e_2;
                env_2.hasError = true;
            }
            finally {
                __disposeResources(env_2);
            }
        }
        return this.deliverToMember(root, childId, content, options);
    }
    /** Admit local input synchronously before any teardown can begin. */
    deliver(activation, message, delivery) {
        if (activation.closing !== undefined) {
            throw new SubagentError('subagent activation is being disposed; the message was not accepted', 'ACTIVATION_CLOSING');
        }
        const local = requireLocalActivation(activation);
        if (local.structured?.acceptsInput() === false) {
            throw new SubagentError('subagent is submitting or has submitted its structured result; the message was not accepted', 'INPUT_CLOSED');
        }
        if (delivery === 'steer')
            local.handle.agent.steer(message);
        else
            local.handle.agent.followup(message);
    }
    /**
     * Reject one child identity already owned by a live Agent or Session.
     * @param childId - proposed durable child session id.
     */
    assertChildIdAvailable(childId) {
        if (this.resident.has(childId) || this.ctx.agents.get(childId) !== undefined || this.ctx.get('sessions')?.get(childId) !== undefined) {
            throw new SubagentError(`subagent "${childId}" already exists`, 'DUPLICATE_CHILD');
        }
    }
    /**
     * Apply activation admission to a resident parent before delivering input.
     * @param parent - exact live Agent receiving the message.
     * @param message - durable user message to deliver.
     * @param delivery - receiving inbox destination.
     */
    sendWaking(parent, message, delivery) {
        const parentActivation = this.resident.get(parent.id);
        if (parentActivation !== undefined && activationAgent(parentActivation) === parent) {
            try {
                this.deliver(parentActivation, message, delivery);
            }
            finally {
                this.wake(parentActivation);
            }
            return;
        }
        if (delivery === 'steer')
            parent.steer(message);
        else
            parent.followup(message);
    }
    /**
     * Reject new work from closing members or subagent trees.
     * @param agent - exact live caller whose instance and subagent tree must admit work.
     */
    assertAdmitting(agent) {
        const closing = this.closingTeardownFor(agent);
        if (closing === undefined) {
            if (this.resident.get(agent.id)?.closing !== undefined) {
                throw new SubagentError(`subagent "${agent.id}" is closing`, 'ACTIVATION_CLOSING');
            }
            return;
        }
        throw new SubagentError(closing === 'manager'
            ? 'subagents are draining; the operation was not admitted'
            : `subagent tree "${closing.id}" is draining; the operation was not admitted`, 'DRAINING');
    }
    /**
     * Create or resume one child Agent and publish its Activation.
     * @param inputs - reconstruction and admission inputs for the residency epoch.
     * @returns the published process-local Activation.
     */
    materialize(inputs) {
        this.assertAdmitting(inputs.root);
        inputs.signal.throwIfAborted();
        const pool = this.rootPool(inputs.root);
        const releaseSlot = pool.reserve(this.maxActiveSubagents());
        return this.materializeTracked(inputs, releaseSlot).catch((error) => {
            releaseSlot();
            throw error;
        });
    }
    /** Keep subagent tree shutdown and host waits joined to preparation, publication, and rollback. */
    trackStartup(root, signal, start) {
        this.assertAdmitting(root);
        signal.throwIfAborted();
        const settled = Promise.withResolvers();
        const controller = new AbortController();
        const materialization = { root, settled: settled.promise, controller };
        this.materializations.add(materialization);
        return start(AbortSignal.any([signal, controller.signal])).catch((error) => {
            if (error instanceof Error && error.name === 'AbortError')
                this.assertAdmitting(root);
            throw error;
        }).finally(() => {
            this.materializations.delete(materialization);
            settled.resolve();
        });
    }
    /**
     * Stop and release one Activation through its memoized close transaction.
     * @param activation - exact residency epoch to close.
     * @returns the shared close transaction.
     */
    dispose(activation) {
        return this.close(activation, () => this.finishDisposal(activation, true));
    }
    /** Close admission synchronously and share one release, including startup rollback. */
    close(activation, release) {
        if (activation.closing !== undefined)
            return activation.closing;
        const completion = Promise.withResolvers();
        activation.closing = completion.promise;
        void release().then(completion.resolve, completion.reject);
        return completion.promise;
    }
    /** Dispose independent roots and report every branch failure after all settle. */
    async disposeRoots(roots, failureSubject) {
        const failures = await Promise.all(roots.map(async (activation) => {
            try {
                await this.dispose(activation);
                return undefined;
            }
            catch (error) {
                return error;
            }
        }));
        const reasons = failures.filter(failure => failure !== undefined);
        if (reasons.length > 0) {
            throw new SubagentError(`subagent teardown failed for ${reasons.length} ${failureSubject}: `
                + reasons.map(reason => failureMessage(reason)).join('; '), 'ACTIVATION_TEARDOWN_FAILED');
        }
    }
    /** Return the subagent tree teardown that closed admission for this exact Agent. */
    closingTeardownFor(agent) {
        if (this.draining)
            return 'manager';
        const root = this.rootOf(agent);
        return this.closingRoots.has(root) ? root : undefined;
    }
    /** Share one capacity pool across every instance in a subagent tree. */
    rootPool(parent) {
        let pool = this.rootPools.get(parent);
        if (pool === undefined) {
            pool = new ActivationPool();
            this.rootPools.set(parent, pool);
        }
        return pool;
    }
    /** Perform one tracked materialization through publication or rollback. */
    async materializeTracked(inputs, releaseSlot) {
        const { provider, root } = inputs;
        inputs.signal.throwIfAborted();
        let resource;
        if (inputs.kind === 'external') {
            const controller = new AbortController();
            const cancelStartup = () => { controller.abort(inputs.signal.reason); };
            inputs.signal.addEventListener('abort', cancelStartup, { once: true });
            try {
                const run = await inputs.start(controller.signal);
                resource = { kind: 'external', run, controller };
                void run.result.catch(() => undefined);
                try {
                    this.assertChildIdAvailable(run.id);
                }
                catch (error) {
                    await run.dispose();
                    throw error;
                }
            }
            finally {
                inputs.signal.removeEventListener('abort', cancelStartup);
            }
        }
        else {
            const { create, childId } = inputs;
            let structured;
            const setup = async (childCtx, child) => {
                // Only fresh creation appends the descriptor and delegated policy after
                // the inherited marker; a cold resume replays those persisted events.
                if (inputs.composition.agentPreset !== undefined) {
                    await applyChildComposition(childCtx, create?.parent, inputs.composition);
                }
                if (create !== undefined) {
                    if (inputs.composition.agentPreset !== undefined) {
                        child.session.append('agent-preset/selected', { agentPreset: inputs.composition.agentPreset });
                    }
                    child.session.append('subagent/descriptor', create.descriptor);
                    appendDelegatedPolicyOverrides(child.session, create.delegatedPolicies);
                    const workingDirectory = childCtx.get('workingDirectory');
                    if (workingDirectory === undefined)
                        throw new Error('local subagents require the working-directory service');
                    await workingDirectory.set(child, create.cwd, inputs.signal);
                }
                if (inputs.composition.agentPreset === undefined)
                    await applyChildComposition(childCtx, create?.parent, inputs.composition);
                if (inputs.outputSchema !== undefined) {
                    structured = attachStructuredRuntime(childCtx, inputs.outputSchema, () => child.inbox.nextStep.length === 0
                        && child.inbox.nextTurn.length === 0);
                }
            };
            const handle = create === undefined
                ? await this.ownerCtx.agents.resume({
                    resumeSessionId: childId,
                    agentOptions: inputs.agentOptions,
                    signal: inputs.signal,
                    setup,
                })
                : await this.ownerCtx.agents.create({
                    sessionId: childId,
                    meta: create.meta,
                    ...(create.seed === undefined ? {} : { seed: create.seed }),
                    inheritedEventCount: create.inheritedEventCount,
                    agentOptions: inputs.agentOptions,
                    signal: inputs.signal,
                    setup,
                });
            resource = {
                kind: 'local',
                handle,
                outputStart: handle.agent.session.seq,
                structured,
                failureAt: undefined,
                poke: Promise.withResolvers(),
                omitClosingText: false,
            };
        }
        const childId = resource.kind === 'local' ? resource.handle.agent.id : resource.run.id;
        const activation = {
            releaseSlot,
            childId,
            runId: SubagentRunId(randomUUID()),
            provider,
            ...resource,
            root,
            ancestors: inputs.ancestors,
            path: inputs.path,
            delivery: inputs.delivery ?? 'parent',
            result: Promise.withResolvers(),
            closing: undefined,
            accepted: false,
        };
        void activation.result.promise.catch(() => undefined);
        this.resident.set(childId, activation);
        try {
            this.assertAdmitting(root);
            inputs.signal.throwIfAborted();
            const wakeSettlement = () => { this.wake(activation); };
            if (activation.kind === 'local') {
                observeLocalFailure(activation, wakeSettlement);
                activation.handle.agent.ctx.on('agent/inbox/inserted', wakeSettlement);
                activation.handle.agent.ctx.on('agent/inbox/claimed', () => {
                    // Claimed input means the closing text may report work the child's
                    // last message to its parent did not.
                    activation.omitClosingText = false;
                    wakeSettlement();
                });
                activation.handle.agent.ctx.on('agent/inbox/discarded', wakeSettlement);
                // A tool call logged after the message also voids it; logged calls
                // cannot be skipped by tool policy. `send_message` is not concurrency
                // safe, so a later call in its step is logged after it executes.
                activation.handle.agent.ctx.on('session/event', (_session, event) => {
                    if (event.type === 'tool/call')
                        activation.omitClosingText = false;
                });
            }
            // Listeners may read the resident Agent or drain it immediately. Publish
            // after residency and inbox listeners, as the last rollback-protected step.
            this.host.emitLifecycle('subagent/start', this.runInfo(activation), root);
        }
        catch (error) {
            /* v8 ignore next -- rollback failure must not mask the admission failure
             * that prevented this operation from returning an accepted message id. */
            await this.rollbackUnpublished(activation, error).catch(() => undefined);
            throw error;
        }
        return activation;
    }
    /** Identity shared by one residency epoch's start/end pair, including cold resumes. */
    runInfo(activation) {
        return {
            runId: activation.runId,
            provider: activation.provider,
            parentSessionId: activation.ancestors.at(-1),
            id: activation.childId,
            local: activation.kind === 'local',
        };
    }
    /** Record acceptance and begin settlement observation before returning the receipt. */
    accept(activation) {
        activation.accepted = true;
        if (activation.kind === 'local') {
            this.watchLocalSettlement(activation);
        }
        else {
            const finish = () => this.close(activation, () => this.finishDisposal(activation, false));
            void activation.run.result.then(finish, finish).catch((error) => { this.reportTeardownFailure(activation, error); });
        }
    }
    /** Release an Activation whose start edge was not published. */
    rollbackUnpublished(activation, error) {
        return this.close(activation, async () => {
            try {
                await (activation.kind === 'local' ? activation.handle.dispose() : activation.run.dispose());
            }
            finally {
                this.resident.delete(activation.childId);
                activation.releaseSlot();
                activation.result.reject(error);
            }
        });
    }
    /** Let a settlement watcher re-check residency after relevant state changes. */
    wake(activation) {
        if (activation.kind === 'external')
            return;
        activation.poke.resolve();
        activation.poke = Promise.withResolvers();
    }
    /** Follow one Activation to natural settlement. */
    watchLocalSettlement(activation) {
        void (async () => {
            while (true) {
                const idleObservation = activation.poke;
                await activation.handle.agent.whenIdle();
                if (activation.closing !== undefined)
                    return;
                const readiness = await this.locks.run(activation.childId, () => Promise.resolve(this.settlementState(activation, idleObservation)));
                if (readiness === 'closed')
                    return;
                if (readiness === 'retry')
                    continue;
                if (readiness === 'wait') {
                    await idleObservation.promise;
                    continue;
                }
                const finalSeq = activation.handle.agent.session.seq;
                await this.flushFinalState(activation);
                const attempt = await this.locks.run(activation.childId, () => {
                    const state = this.settlementState(activation, idleObservation);
                    if (state !== 'ready')
                        return Promise.resolve(state);
                    if (activation.handle.agent.session.seq !== finalSeq) {
                        return Promise.resolve('retry');
                    }
                    let done;
                    try {
                        void activation.handle.agent.runMaintenance(() => {
                            done = this.close(activation, () => this.finishDisposal(activation, false));
                            return Promise.resolve();
                        });
                    }
                    catch (_error) {
                        // Another activity already owns the Agent's idle phase.
                        return Promise.resolve('retry');
                    }
                    return Promise.resolve({ done });
                });
                if (typeof attempt === 'string') {
                    if (attempt === 'closed')
                        return;
                    continue;
                }
                try {
                    await attempt.done;
                }
                catch (error) {
                    this.reportTeardownFailure(activation, error);
                }
                return;
            }
        })();
    }
    /** Report disposal failure once after the natural settlement observer finishes. */
    reportTeardownFailure(activation, error) {
        this.ctx.logger.warn(`subagent "${activation.childId}" activation teardown failed: ${failureMessage(error)}`);
    }
    /** Check pending input before closing admission. */
    settlementState(activation, observation) {
        if (activation.closing !== undefined)
            return 'closed';
        if (activation.poke !== observation)
            return 'retry';
        const inbox = activation.handle.agent.inbox;
        if (activation.failureAt === undefined && (inbox.nextTurn.length > 0 || inbox.nextStep.length > 0))
            return 'wait';
        return 'ready';
    }
    /** Stop and release one instance without affecting its creation descendants. */
    async finishDisposal(activation, stop) {
        this.wake(activation);
        const { childId } = activation;
        const failures = [];
        let result = { output: [], stopReason: 'error' };
        let resultFailure;
        let externalDisposal;
        const child = activationAgent(activation);
        const detail = (error) => child === undefined
            ? failureMessage(error)
            : errorChain(error);
        try {
            if (activation.kind === 'external') {
                if (stop) {
                    activation.controller.abort({ kind: 'parent' });
                    // Some backends settle their result only when disposal begins.
                    externalDisposal = Promise.resolve().then(() => activation.run.dispose());
                    void externalDisposal.catch(() => undefined);
                }
                result = await activation.run.result;
            }
            else {
                if (stop) {
                    activation.handle.agent.cancel({ kind: 'parent' });
                    const idle = activation.handle.agent.whenIdle();
                    await idle;
                    await this.flushFinalState(activation);
                }
                result = captureLocalResult(activation);
            }
        }
        catch (error) {
            resultFailure = { error };
            failures.push(new SubagentError(`subagent "${childId}" activation teardown failed: ${detail(error)}`, 'ACTIVATION_TEARDOWN_FAILED', { cause: error }));
        }
        try {
            if (activation.kind === 'local')
                await activation.handle.dispose();
            else
                await (externalDisposal ?? activation.run.dispose());
        }
        catch (error) {
            failures.push(new SubagentError(`subagent "${childId}" activation handle disposal failed: ${detail(error)}`, 'ACTIVATION_TEARDOWN_FAILED', { cause: error }));
        }
        let failure;
        if (failures.length === 1) {
            failure = failures[0];
        }
        else if (failures.length > 1) {
            failure = new SubagentError(`subagent "${childId}" activation teardown failed at ${failures.length} boundaries: `
                + failures.map(item => detail(item)).join('; '), 'ACTIVATION_TEARDOWN_FAILED', { cause: new AggregateError(failures) });
        }
        if (failure !== undefined && result.stopReason === 'completed' && child !== undefined
            && (child.inbox.nextStep.length > 0 || child.inbox.nextTurn.length > 0)) {
            result = { ...result, stopReason: 'error' };
        }
        this.resident.delete(childId);
        activation.releaseSlot();
        const terminal = failure === undefined ? result : { output: [], stopReason: 'error' };
        this.notifySettlement(activation, result);
        // The sole end publication follows resource release and parent notification,
        // before result delivery.
        this.host.emitLifecycle('subagent/end', {
            ...this.runInfo(activation),
            stopReason: terminal.stopReason,
            ...terminal.output.length === 0 ? {} : { lastAssistantMessage: terminal.output },
        }, activation.root);
        if (resultFailure === undefined)
            activation.result.resolve(result);
        else
            activation.result.reject(resultFailure.error);
        if (failure !== undefined)
            throw failure;
    }
    /** Deliver the captured execution result to the durable direct parent after cleanup. */
    notifySettlement(activation, terminal) {
        if (!activation.accepted || activation.delivery === 'caller')
            return;
        try {
            const parentId = activation.ancestors.at(-1);
            const parent = this.ctx.agents.get(parentId);
            if (parent === undefined || parent.ctx.fiber.state !== 2 /* FiberState.ACTIVE */
                || this.rootOf(parent) !== activation.root)
                return;
            const message = createSettlementMessage(activation.childId, agentAddress(activation.path, activation.childId), terminal, activation.kind === 'local', activation.kind === 'local' && activation.omitClosingText);
            if (this.closingTeardownFor(parent) !== undefined) {
                parent.inject(message);
                return;
            }
            this.sendWaking(parent, message, parent.status === 'idle' ? 'queue' : 'steer');
        }
        catch (error) {
            this.ctx.logger.warn(`subagent "${activation.childId}" settlement notice was not delivered to its parent: `
                + errorChain(error));
        }
    }
    /** Request a best-effort final session flush before closing natural-settlement admission. */
    async flushFinalState(activation) {
        try {
            await activation.handle.agent.ctx.sessions.flush(activation.handle.agent.session);
        }
        catch (error) {
            this.ctx.logger.warn(`subagent "${activation.childId}" best-effort final session flush failed; `
                + `the persisted state may be unavailable or stale on resume: ${errorChain(error)}`);
        }
    }
    /** Return capabilities bound to an exact activation instead of a future same-id instance. */
    receipt(activation) {
        return {
            childId: activation.childId,
            name: agentAddress(activation.path, activation.childId),
            result: activation.result.promise,
            dispose: () => this.dispose(activation),
        };
    }
    /** Serialize target restoration and inbox admission; closure is awaited outside the lock. */
    async deliverToMember(root, childId, content, options) {
        while (true) {
            this.assertAdmitting(root);
            options.signal.throwIfAborted();
            const result = await this.locks.run(childId, async () => {
                if (childId === root.id) {
                    if (contentHasImage(content))
                        await this.assertImageCapable(root, options.signal);
                    options.signal.throwIfAborted();
                    options.authorize?.();
                    this.assertAdmitting(root);
                    const message = createUserMessage({ content, source: options.source });
                    this.sendWaking(root, message, options.delivery);
                    options.onAccepted?.();
                    return { messageId: message.id };
                }
                const activation = this.resident.get(childId);
                if (activation === undefined) {
                    const restored = await this.restoreTarget(root, childId, options.signal);
                    return { messageId: await this.submitMaterialized(restored, content, options, root) };
                }
                if (activation.root !== root)
                    throw new SubagentError('target belongs to another subagent tree', 'UNAUTHORIZED');
                requireLocalActivation(activation);
                if (activation.closing === undefined && contentHasImage(content)) {
                    await this.assertImageCapable(requireLocalActivation(activation).handle.agent, options.signal);
                }
                if (activation.closing !== undefined)
                    return { closing: activation.closing };
                const messageId = this.submitAdmitted(activation, content, options, root);
                return { messageId };
            });
            if ('messageId' in result)
                return result.messageId;
            await Promise.allSettled([result.closing]);
        }
    }
    /** Read continuation history, preserving cancellation and reporting unavailable storage. */
    async observeContinuation(childId, signal) {
        const query = this.requireSessionQuery();
        try {
            return await query.observeSession(childId, { signal });
        }
        catch (error) {
            signal.throwIfAborted();
            throw new SubagentError(`subagent "${childId}" is unavailable`, 'NOT_RESUMABLE', { cause: error });
        }
    }
    /** Reconstruct only the target under its subagent tree root; the caller owns acceptance or rollback. */
    async restoreTarget(root, childId, signal) {
        return this.trackStartup(root, signal, async (signal) => {
            const env_3 = { stack: [], error: void 0, hasError: false };
            try {
                const source = __addDisposableResource(env_3, await this.observeContinuation(childId, signal), false);
                this.assertAdmitting(root);
                const ownEvents = source.events.slice(source.inheritedEventCount);
                const descriptor = foldSubagentDescriptor(ownEvents);
                if (descriptor === undefined || descriptor.mode !== 'continuable') {
                    throw new SubagentError(`subagent "${childId}" has no supported continuation state and cannot be resumed; choose a different target`, 'NOT_RESUMABLE');
                }
                const { ancestors, path } = await this.resolveCreationAncestry(source, signal, root);
                let activation;
                try {
                    const projectedPreset = source.projections?.values.agentPreset;
                    if (projectedPreset === undefined && source.events.some(event => event.type === 'agent-preset/selected')) {
                        throw new SubagentError(`subagent "${childId}" has no agent preset projection for restoration`, 'NOT_RESUMABLE');
                    }
                    const agentPreset = projectedPreset ?? source.header.agentPreset;
                    activation = await this.materialize({
                        kind: 'local',
                        childId,
                        provider: descriptor.provider,
                        root,
                        ancestors,
                        path,
                        agentOptions: {
                            ...descriptor.agentProvider !== undefined ? { provider: descriptor.agentProvider } : {},
                            ...descriptor.agentModel !== undefined ? { model: descriptor.agentModel } : {},
                            ...descriptor.agentReasoningEffort !== undefined
                                ? { reasoningEffort: ReasoningEffortId(descriptor.agentReasoningEffort) }
                                : {},
                        },
                        composition: {
                            agentPreset: agentPreset ?? undefined,
                            persona: descriptor.persona,
                            toolFilter: descriptor.toolFilter,
                        },
                        signal,
                    });
                }
                catch (error) {
                    signal.throwIfAborted();
                    if (error instanceof SubagentError)
                        throw error;
                    throw new SubagentError(`subagent "${childId}" is unavailable`, 'NOT_RESUMABLE', { cause: error });
                }
                return activation;
            }
            catch (e_3) {
                env_3.error = e_3;
                env_3.hasError = true;
            }
            finally {
                __disposeResources(env_3);
            }
        });
    }
    /** Admit a materialized child, commit its creation fact, and release it on failure. */
    async submitMaterialized(activation, content, options, parent, commit) {
        try {
            if (contentHasImage(content)) {
                await this.assertImageCapable(requireLocalActivation(activation).handle.agent, options.signal);
                if (activation.closing !== undefined) {
                    throw new SubagentError(`subagent "${activation.childId}" is closing`, 'ACTIVATION_CLOSING');
                }
            }
            const messageId = this.submitAdmitted(activation, content, options, parent);
            commit?.();
            this.accept(activation);
            return messageId;
        }
        catch (error) {
            try {
                await this.dispose(activation);
            }
            catch (cleanupError) {
                this.ctx.logger.warn(`subagent continuation: disposal after admission or catalog append failure also failed: ${String(cleanupError)}`);
            }
            throw error;
        }
    }
    /** Build and submit one message across the final synchronous admission cutoff. */
    submitAdmitted(activation, content, options, parent) {
        const message = createUserMessage({ content, source: options.source });
        options.signal.throwIfAborted();
        options.authorize?.();
        this.assertAdmitting(parent);
        try {
            this.deliver(activation, message, options.delivery);
        }
        finally {
            this.wake(activation);
        }
        options.onAccepted?.();
        return message.id;
    }
    /** Refuse image content for a child whose fixed model accepts text only. */
    async assertImageCapable(agent, signal) {
        const { provider, model } = agent.options;
        if (provider === undefined || model === undefined)
            return;
        const llm = this.ctx.get('llm');
        /* v8 ignore next -- without an LLM registry, delivery defers to projection. */
        if (llm === undefined)
            return;
        const info = await llm.resolveModelInfo(provider, model, signal);
        if (info.inputModalities !== undefined && !info.inputModalities.includes('image')) {
            throw new SubagentError(`Model "${model}" does not support image input.`, 'MODEL_DOES_NOT_SUPPORT_IMAGES');
        }
    }
    /** Resolve the persistence service continuable children require, or fail loud. */
    requirePersistence() {
        const persistence = this.ctx.get('sessionPersistence');
        if (persistence === undefined) {
            throw new SubagentError('continuable subagents require session persistence (load a dsh-session-persistence backend)', 'PERSISTENCE_UNAVAILABLE');
        }
        return persistence;
    }
    /** Resolve the Session query service used for cold child observations. */
    requireSessionQuery() {
        const query = this.ctx.get('sessionQuery');
        if (query === undefined) {
            throw new SubagentError('continuable subagents require session query (load @deepseek-ai/dsh-session-query)', 'CONTINUATION_UNAVAILABLE');
        }
        return query;
    }
}
export default SubagentManager;
//# sourceMappingURL=manager.js.map