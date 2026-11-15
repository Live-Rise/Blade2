/** Shared task boards owned by subagent tree roots. */
var __runInitializers = (this && this.__runInitializers) || function (thisArg, initializers, value) {
    var useValue = arguments.length > 2;
    for (var i = 0; i < initializers.length; i++) {
        value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
    }
    return useValue ? value : void 0;
};
var __esDecorate = (this && this.__esDecorate) || function (ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
    function accept(f) { if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected"); return f; }
    var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
    var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
    var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
    var _, done = false;
    for (var i = decorators.length - 1; i >= 0; i--) {
        var context = {};
        for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
        for (var p in contextIn.access) context.access[p] = contextIn.access[p];
        context.addInitializer = function (f) { if (done) throw new TypeError("Cannot add initializers after decoration has completed"); extraInitializers.push(accept(f || null)); };
        var result = (0, decorators[i])(kind === "accessor" ? { get: descriptor.get, set: descriptor.set } : descriptor[key], context);
        if (kind === "accessor") {
            if (result === void 0) continue;
            if (result === null || typeof result !== "object") throw new TypeError("Object expected");
            if (_ = accept(result.get)) descriptor.get = _;
            if (_ = accept(result.set)) descriptor.set = _;
            if (_ = accept(result.init)) initializers.unshift(_);
        }
        else if (_ = accept(result)) {
            if (kind === "field") initializers.unshift(_);
            else descriptor[key] = _;
        }
    }
    if (target) Object.defineProperty(target, contextIn.name, descriptor);
    done = true;
};
import { Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { Remote, TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import { TeamError } from "./error.js";
import { TeamJournal } from "./journal.js";
import { teamProjectionDefinition } from "./projection.js";
import { TeamTaskBoard } from "./task-board.js";
export { TeamId, TeamMessageId, TeamTaskId } from "./types.js";
export { TeamError } from "./error.js";
const DEFAULT_MAX_TASKS = 256;
/** Configuration keys removed when Team reused subagent creation and messaging. */
const REMOVED_CONFIG_KEYS = ['maxMembers', 'maxMessageBytes', 'disposalTimeoutMs'];
/** Validate one positive safe-integer deployment limit. */
function positiveLimit(name, value) {
    if (!Number.isSafeInteger(value) || value < 1) {
        throw new TeamError(`${name} must be a positive safe integer`, 'TEAM_INVALID_CONFIG');
    }
    return value;
}
/** Agent Teams service backed by the exact live Lead Session log. */
let TeamService = (() => {
    let _classSuper = TypertRemoteService;
    let _instanceExtraInitializers = [];
    let _setEnabled_decorators;
    return class TeamService extends _classSuper {
        static {
            const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
            _setEnabled_decorators = [Remote];
            __esDecorate(this, null, _setEnabled_decorators, { kind: "method", name: "setEnabled", static: false, private: false, access: { has: obj => "setEnabled" in obj, get: obj => obj.setEnabled }, metadata: _metadata }, null, _instanceExtraInitializers);
            if (_metadata) Object.defineProperty(this, Symbol.metadata, { enumerable: true, configurable: true, writable: true, value: _metadata });
        }
        static inject = ['agents', 'sessions', 'sessionPersistence', 'sessionProjections', 'subagents'];
        static Config = z.object({
            maxTasks: z.number().step(1).min(1).default(DEFAULT_MAX_TASKS),
            defaultEnabled: z.boolean().default(false),
        });
        projection = __runInitializers(this, _instanceExtraInitializers);
        maxTasks;
        defaultEnabled;
        journal;
        tasks;
        constructor(ctx, config = {}) {
            super(ctx, 'agentTeams');
            for (const key of REMOVED_CONFIG_KEYS) {
                if (Object.hasOwn(config, key)) {
                    throw new TeamError(`agent-team: ${key} is no longer supported; teammates are subagents, so subagent limits and inbox delivery apply`, 'TEAM_INVALID_CONFIG');
                }
            }
            this.maxTasks = positiveLimit('maxTasks', config.maxTasks ?? DEFAULT_MAX_TASKS);
            this.defaultEnabled = config.defaultEnabled ?? false;
        }
        async [Service.init]() {
            const ctx = this.ctx;
            this.projection = await ctx.sessionProjections.register(teamProjectionDefinition);
            this.journal = new TeamJournal(ctx, this.projection);
            this.tasks = new TeamTaskBoard(ctx, this.journal, this.maxTasks);
            if (this.defaultEnabled) {
                ctx.on('agent/created', async ({ agent, source, signal }) => {
                    // Child activation ownership is registered after agent/created settles.
                    if (source !== 'startup' || agent.session.header.origin === 'subagent')
                        return;
                    if (this.projection.stateOf(agent.session).enabled !== undefined)
                        return;
                    await this.setEnabled(agent, true, signal);
                });
            }
        }
        /**
         * Read the Team selection shared by the caller's subagent tree.
         * @param caller - exact live root or descendant.
         * @returns whether Team tools and guidance are enabled.
         */
        isEnabled(caller) {
            const root = this.ctx.subagents.rootOf(caller);
            return this.projection.stateOf(root.session).enabled === true;
        }
        /**
         * Save a root Session's Team selection without stopping children or deleting tasks.
         * The selection takes effect before awaiting persistence. Tools resolve availability at
         * step preparation; prompt providers read the selection when evaluated. Task mutations
         * check the current mode when their queued operation starts; admitted operations finish.
         * @param agent - exact live root whose tree receives the selection.
         * @param enabled - selected Team mode.
         * @param signal - cancellation checked before changing the selection.
         * @returns fulfillment after the selection flushes.
         * @throws when called by a descendant or persistence fails. An appended selection remains active;
         * retrying the same selection flushes without a duplicate event.
         */
        async setEnabled(agent, enabled, signal) {
            const root = this.ctx.subagents.rootOf(agent);
            if (root !== agent)
                throw new TeamError('change Team mode from the root Session', 'TEAM_LEAD_REQUIRED');
            signal?.throwIfAborted();
            if (this.isEnabled(root) !== enabled)
                root.session.append('team/mode', { enabled });
            await this.ctx.sessions.flush(root.session);
        }
        /**
         * Create one unowned pending task in the Team Lead log.
         * @param caller - exact live Team member creating the task.
         * @param request - task text, blockers, and advisory write scopes.
         * @param signal - cancellation before the task event is appended; omitted means no cancellation.
         * @returns the revision-one task view.
         * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED when the task is visible but durability is unknown; read it before retrying.
         */
        async createTask(caller, request, signal) {
            return await this.tasks.create(caller, this.ctx.subagents.rootOf(caller), request, signal);
        }
        /**
         * Return one task, including a deleted tombstone.
         * @param caller - exact live Team member reading the task.
         * @param id - Team-local task identity.
         * @returns the latest task value and derived readiness diagnostics.
         */
        getTask(caller, id) {
            return this.tasks.get(this.ctx.subagents.rootOf(caller), id);
        }
        /**
         * List current non-deleted tasks in numeric creation order.
         * @param caller - exact live Team member reading the board.
         * @returns detached current task views.
         */
        listTasks(caller) {
            return this.tasks.list(this.ctx.subagents.rootOf(caller));
        }
        /**
         * Compare-and-set one authorized task transition.
         * @param caller - exact live Team member authorizing the mutation.
         * @param request - task identity, expected revision, action, and action fields.
         * @param signal - cancellation before the task event is appended; omitted means no cancellation.
         * @returns the committed next task revision.
         * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED when the task is visible but durability is unknown; read it before retrying.
         */
        async updateTask(caller, request, signal) {
            return await this.tasks.update(caller, this.ctx.subagents.rootOf(caller), request, signal);
        }
    };
})();
export { TeamService };
export default TeamService;
//# sourceMappingURL=index.js.map