/** Session Remote owner: cold reads, explicit Agent commands, and live control state. */
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
import { hostname } from 'node:os';
import { resolve } from 'node:path';
import { Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { errorChain, ReasoningEffortId } from '@deepseek-ai/dsh-llm';
import { canOpenNativePath, nativeFileManager, nativeFileApplications, openNativeFileApplication, openNativeAssociatedPath, revealNativePath } from '@deepseek-ai/dsh-native-command';
import { Remote, RemoteError, TypertRemoteService, principalOf } from '@deepseek-ai/dsh-typert-protocol';
import { canonicalClientTimeZone } from '@deepseek-ai/dsh-util-time';
import { assertNever } from '@deepseek-ai/dsh-util-values';
import { ApiSessionAgentController, } from "./agent.js";
import { SessionCommandController, hasPromptContent } from "./commands.js";
import { SessionControlController } from "./control.js";
import { HistoryController } from "./history/controller.js";
import { StateController } from "./state/controller.js";
import { WebController } from "./web/controller.js";
import { SessionFileReferences } from "./file-references.js";
import { ApiSessionList } from "./list.js";
import { buildModelCatalog, hasProviderApiKey } from "./catalog.js";
import { SessionSkillCatalog } from "./skill-catalog.js";
import { SessionMediaReferences } from "./media-references.js";
import { ArchivedSessionGate } from "./archived-session-gate.js";
export { ApiSessionNotFound } from "./agent.js";
export { SessionFileReferences } from "./file-references.js";
export { SessionSkillCatalog } from "./skill-catalog.js";
/**
 * Extract the Session resource from an ID or a history address.
 *
 * Registers as the `session` kind default, so it applies to every Remote method of
 * every namespace: a top-level `sessionId` parameter, otherwise the first object
 * parameter carrying `sessionId` or an `address`. A new request type whose first
 * object parameter has an unrelated `address` field would be read as an address.
 * @param input - validated wire arguments and their ordered parameter descriptions.
 * @returns the named Session identity, otherwise the first object's Session identity or history address.
 * @throws Error when the request object supplies both sessionId and address.
 */
function resolveSessionResource({ args, parameters }) {
    const sessionParameter = parameters.find(parameter => parameter.name === 'sessionId' || parameter.wire === 'sessionId');
    if (sessionParameter !== undefined)
        return args[sessionParameter.wire];
    for (const parameter of parameters) {
        const value = args[parameter.wire];
        if (typeof value !== 'object' || value === null || Array.isArray(value))
            continue;
        const request = value;
        const address = request.address;
        if (request.sessionId !== undefined && address !== undefined) {
            throw new Error('Session resource request must not contain both sessionId and address');
        }
        return request.sessionId ?? (address === undefined ? undefined
            : address.kind === 'session' ? address.sessionId : address.childSessionId);
    }
    return undefined;
}
/** Host service backing the generated `ctx.remote.session` namespace. */
let SessionController = (() => {
    let _classSuper = TypertRemoteService;
    let _instanceExtraInitializers = [];
    let _list_decorators;
    let _search_decorators;
    let _create_decorators;
    let _selectModel_decorators;
    let _initializeDefaultModel_decorators;
    let _modelCatalog_decorators;
    let _canOpenWorkspacePath_decorators;
    let _openWorkspacePath_decorators;
    let _workspacePathApplications_decorators;
    let _rename_decorators;
    let _fork_decorators;
    let _prompt_decorators;
    let _attachment_decorators;
    let _updateQueue_decorators;
    let _cancel_decorators;
    let _page_decorators;
    let _events_decorators;
    let _observeEvent_decorators;
    let _observeProjections_decorators;
    let _state_decorators;
    let _observeState_decorators;
    let _wait_decorators;
    let _follow_decorators;
    let _projections_decorators;
    let _control_decorators;
    return class SessionController extends _classSuper {
        static {
            const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
            _list_decorators = [Remote('list')];
            _search_decorators = [Remote('search')];
            _create_decorators = [Remote('create')];
            _selectModel_decorators = [Remote('selectModel')];
            _initializeDefaultModel_decorators = [Remote];
            _modelCatalog_decorators = [Remote('modelCatalog')];
            _canOpenWorkspacePath_decorators = [Remote];
            _openWorkspacePath_decorators = [Remote('openWorkspacePath')];
            _workspacePathApplications_decorators = [Remote('workspacePathApplications')];
            _rename_decorators = [Remote('rename')];
            _fork_decorators = [Remote('fork')];
            _prompt_decorators = [Remote('prompt')];
            _attachment_decorators = [Remote('attachment')];
            _updateQueue_decorators = [Remote('updateQueue')];
            _cancel_decorators = [Remote('cancel')];
            _page_decorators = [Remote('page')];
            _events_decorators = [Remote];
            _observeEvent_decorators = [Remote({ mode: 'stream' })];
            _observeProjections_decorators = [Remote({ mode: 'stream' })];
            _state_decorators = [Remote];
            _observeState_decorators = [Remote({ mode: 'stream' })];
            _wait_decorators = [Remote];
            _follow_decorators = [Remote({ mode: 'stream' })];
            _projections_decorators = [Remote('projections')];
            _control_decorators = [Remote({ mode: 'stream' })];
            __esDecorate(this, null, _list_decorators, { kind: "method", name: "list", static: false, private: false, access: { has: obj => "list" in obj, get: obj => obj.list }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _search_decorators, { kind: "method", name: "search", static: false, private: false, access: { has: obj => "search" in obj, get: obj => obj.search }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _create_decorators, { kind: "method", name: "create", static: false, private: false, access: { has: obj => "create" in obj, get: obj => obj.create }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _selectModel_decorators, { kind: "method", name: "selectModel", static: false, private: false, access: { has: obj => "selectModel" in obj, get: obj => obj.selectModel }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _initializeDefaultModel_decorators, { kind: "method", name: "initializeDefaultModel", static: false, private: false, access: { has: obj => "initializeDefaultModel" in obj, get: obj => obj.initializeDefaultModel }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _modelCatalog_decorators, { kind: "method", name: "modelCatalog", static: false, private: false, access: { has: obj => "modelCatalog" in obj, get: obj => obj.modelCatalog }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _canOpenWorkspacePath_decorators, { kind: "method", name: "canOpenWorkspacePath", static: false, private: false, access: { has: obj => "canOpenWorkspacePath" in obj, get: obj => obj.canOpenWorkspacePath }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _openWorkspacePath_decorators, { kind: "method", name: "openWorkspacePath", static: false, private: false, access: { has: obj => "openWorkspacePath" in obj, get: obj => obj.openWorkspacePath }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _workspacePathApplications_decorators, { kind: "method", name: "workspacePathApplications", static: false, private: false, access: { has: obj => "workspacePathApplications" in obj, get: obj => obj.workspacePathApplications }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _rename_decorators, { kind: "method", name: "rename", static: false, private: false, access: { has: obj => "rename" in obj, get: obj => obj.rename }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _fork_decorators, { kind: "method", name: "fork", static: false, private: false, access: { has: obj => "fork" in obj, get: obj => obj.fork }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _prompt_decorators, { kind: "method", name: "prompt", static: false, private: false, access: { has: obj => "prompt" in obj, get: obj => obj.prompt }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _attachment_decorators, { kind: "method", name: "attachment", static: false, private: false, access: { has: obj => "attachment" in obj, get: obj => obj.attachment }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _updateQueue_decorators, { kind: "method", name: "updateQueue", static: false, private: false, access: { has: obj => "updateQueue" in obj, get: obj => obj.updateQueue }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _cancel_decorators, { kind: "method", name: "cancel", static: false, private: false, access: { has: obj => "cancel" in obj, get: obj => obj.cancel }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _page_decorators, { kind: "method", name: "page", static: false, private: false, access: { has: obj => "page" in obj, get: obj => obj.page }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _events_decorators, { kind: "method", name: "events", static: false, private: false, access: { has: obj => "events" in obj, get: obj => obj.events }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _observeEvent_decorators, { kind: "method", name: "observeEvent", static: false, private: false, access: { has: obj => "observeEvent" in obj, get: obj => obj.observeEvent }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _observeProjections_decorators, { kind: "method", name: "observeProjections", static: false, private: false, access: { has: obj => "observeProjections" in obj, get: obj => obj.observeProjections }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _state_decorators, { kind: "method", name: "state", static: false, private: false, access: { has: obj => "state" in obj, get: obj => obj.state }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _observeState_decorators, { kind: "method", name: "observeState", static: false, private: false, access: { has: obj => "observeState" in obj, get: obj => obj.observeState }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _wait_decorators, { kind: "method", name: "wait", static: false, private: false, access: { has: obj => "wait" in obj, get: obj => obj.wait }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _follow_decorators, { kind: "method", name: "follow", static: false, private: false, access: { has: obj => "follow" in obj, get: obj => obj.follow }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _projections_decorators, { kind: "method", name: "projections", static: false, private: false, access: { has: obj => "projections" in obj, get: obj => obj.projections }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _control_decorators, { kind: "method", name: "control", static: false, private: false, access: { has: obj => "control" in obj, get: obj => obj.control }, metadata: _metadata }, null, _instanceExtraInitializers);
            if (_metadata) Object.defineProperty(this, Symbol.metadata, { enumerable: true, configurable: true, writable: true, value: _metadata });
        }
        static inject = [
            'agentDefaultModel',
            'agents',
            'attachments',
            'fileUploads',
            'fs',
            'llm',
            'remoteAccess',
            'sessions',
            'sessionProjections',
            'sessionQuery',
            'typert',
            'typertGateway',
            'workspaceRegistry',
        ];
        static Config = z.object({
            nativeOpen: z.boolean(),
            listWorkSliceMs: z.natural().min(1).default(16),
        });
        historyController = __runInitializers(this, _instanceExtraInitializers);
        stateController;
        webController;
        agents;
        commands;
        controlState;
        listState;
        openPath;
        fileApplications;
        openFileApplication;
        revealPath;
        canOpenPath;
        listWorkSliceMs;
        promotions = new Set();
        /**
         * @param ctx - Host context containing the Session capability assembly.
         * @param config - native-opener and list-scheduling deployment policy.
         * @param internals - host integrations replaceable by direct unit tests.
         */
        constructor(ctx, config, internals = {}) {
            super(ctx, 'sessionController', { namespace: 'session' });
            this.listWorkSliceMs = SessionController.Config(config).listWorkSliceMs;
            this.fileApplications = internals.fileApplications ?? nativeFileApplications;
            this.openFileApplication = internals.openFileApplication ?? openNativeFileApplication;
            this.openPath = internals.openPath ?? openNativeAssociatedPath;
            this.revealPath = internals.revealPath ?? revealNativePath;
            this.canOpenPath = internals.canOpenPath
                ?? (() => config.nativeOpen ?? (internals.openPath !== undefined || canOpenNativePath()));
        }
        async [Service.init]() {
            const ctx = this.ctx;
            ctx.typert.resources.register('session', { resolve: resolveSessionResource });
            this.agents = new ApiSessionAgentController(ctx);
            this.agents.initialize();
            this.commands = new SessionCommandController(ctx, this.agents, process.cwd());
            ctx.effect(() => ctx.fileUploads.registerAgentResolver(async (sessionId) => {
                const result = await this.agents.resolveAgent(sessionId);
                if ('error' in result)
                    throw result.error;
                return result.agent;
            }), 'session-controller: file-upload Agent resolver');
            this.controlState = new SessionControlController(ctx);
            // Registered before history so reverse-order teardown closes every
            // follower before waiting for already-admitted promotions.
            ctx.effect(() => async () => {
                await Promise.allSettled([...this.promotions]);
            }, 'session-controller.promotions');
            // Assistant-stream observation must be live before the list read yields: an Agent's
            // opening frame arriving during that await would otherwise have no listener.
            this.historyController = new HistoryController(ctx);
            this.stateController = new StateController(ctx, this.historyController);
            this.webController = new WebController(ctx, this.historyController, (observation) => { this.promote(observation); });
            this.listState = new ApiSessionList(ctx, this.listWorkSliceMs);
            await this.listState.initialize();
            ctx.plugin(SessionFileReferences);
            ctx.plugin(SessionMediaReferences);
            ctx.plugin(SessionSkillCatalog);
            // An archived Session, or a subagent descendant of one, runs no model step
            // until it is restored; what it still runs is stopped by the owners that
            // answer the Workspace registry's archive-admission events.
            ctx.plugin(ArchivedSessionGate);
            ctx.on('session/created', (session) => {
                ctx.emit('api-session/added', this.listState.summaryFor(session));
            });
            ctx.on('session/disposed', (session) => {
                ctx.emit('api-session/removed', session.id);
            });
            const publishAgentAvailability = ({ agent }) => {
                if (ctx.sessions.get(agent.id) === agent.session) {
                    ctx.emit('api-session/added', this.listState.summaryFor(agent.session));
                }
            };
            ctx.on('agent/created', publishAgentAvailability);
            ctx.on('agent/disposed', publishAgentAvailability);
            ctx.on('agent/status', ({ agent, status }) => {
                ctx.emit('api-session/status', agent.id, status === 'running');
            });
            ctx.on('agent/error', ({ agent, error }) => {
                ctx.emit('api-session/error', agent.id, errorChain(error));
            });
            ctx.on('session/event', (session, event) => {
                if (event.type !== 'user/message' || event.data.source.kind !== 'user')
                    return;
                ctx.emit('api-session/activity', session.id, event.time);
            });
        }
        /**
         * Request background activation through the existing Session Agent controller.
         * @param observation - owned lease disposed by the background task; callers retain their own lease separately.
         */
        promote(observation) {
            const sessionId = observation.header.id;
            const task = (async () => {
                const env_1 = { stack: [], error: void 0, hasError: false };
                try {
                    const ownedObservation = __addDisposableResource(env_1, observation, false);
                    const result = await this.agents.resolveObservedAgent(ownedObservation);
                    if ('error' in result)
                        this.ctx.emit('api-session/error', sessionId, result.error.message);
                }
                catch (e_1) {
                    env_1.error = e_1;
                    env_1.hasError = true;
                }
                finally {
                    __disposeResources(env_1);
                }
            })().catch((error) => {
                this.ctx.logger.error(`session-controller: background activation for "${sessionId}" failed: ${errorChain(error)}`);
            });
            this.promotions.add(task);
            void task.finally(() => { this.promotions.delete(task); });
        }
        /**
         * Resolve or resume one ordinary Session for another Host API domain.
         * @param sessionId - Session identity whose Agent owns the operation.
         * @returns the live Agent or the stable Session-domain failure.
         */
        resolveAgent(sessionId) {
            return this.agents.resolveAgent(sessionId);
        }
        /**
         * Read all visible Session rows without resuming an Agent.
         * @param _request - reserved empty list request.
         * @param signal - cancellation for persistence reads and summary generation.
         * @returns visible Session summaries ordered by activity.
         */
        async list(_request, signal) {
            const items = await this.listState.list(signal);
            return { items: items.filter(item => this.ctx.remoteAccess.allows({ kind: 'session', id: item.sessionId })) };
        }
        /**
         * Search visible Session content without resuming an Agent. A non-owner
         * receives only the hits whose Session `allows()` admits; `hasMore`
         * describes the bounded result before that filtering.
         * @param request - literal message-content query.
         * @param signal - cancellation for list and search reads.
         * @returns authorized bounded Session search results.
         */
        async search(request, signal) {
            const result = await this.listState.search(request.query, signal);
            return { ...result, items: result.items.filter(item => this.ctx.remoteAccess.allows({ kind: 'session', id: item.sessionId })) };
        }
        /**
         * Create one ordinary Session for a new conversation, reusing the named blank Session while it is reusable.
         * @param request - optional blank Session to reuse, location, and Agent preset.
         * @returns the created or reused Session identity and its preset when configured.
         */
        create(request) {
            return this.commands.create(request);
        }
        /**
         * Select one Session-local model after explicitly resuming the Session; save the default in the background.
         * @param request - Session identity and requested model selection.
         * @returns the normalized selection installed for the Session, without waiting for default persistence.
         */
        selectModel(request) {
            return this.commands.selectModel(request);
        }
        /**
         * Select the first available account model after login when no provider API key is configured.
         * @returns after saving the first available model or retaining the existing default.
         */
        async initializeDefaultModel() {
            const provider = 'deepseek-account';
            if (await hasProviderApiKey(this.ctx))
                return;
            const catalog = await buildModelCatalog(this.ctx);
            const model = catalog.groups.find(group => group.id === provider)?.models[0];
            if (model === undefined)
                throw new RemoteError('session/provider-models-unavailable', `provider "${provider}" has no available models`, { provider });
            const selection = { provider, model: model.id,
                ...model.reasoning?.defaultEffort === undefined ? {} : { reasoningEffort: ReasoningEffortId(model.reasoning.defaultEffort) },
            };
            await this.ctx.agentDefaultModel.saveSelection(selection);
        }
        /**
         * Describe every currently routable model for Host-generation selectors.
         * @returns provider-grouped models, the deployment default, and isolated provider failures.
         */
        modelCatalog() {
            return buildModelCatalog(this.ctx);
        }
        /**
         * Report whether this deployment can hand a Session workspace path to a native desktop.
         * @returns true when the matching open operation is available.
         */
        canOpenWorkspacePath() {
            return this.canOpenPath();
        }
        /**
         * Describe the serving desktop for authenticated file-action routes.
         * @returns Host name, configured availability, and platform-specific file-manager behavior.
         */
        workspaceDesktop() {
            const fileManager = nativeFileManager();
            return { name: hostname(), available: fileManager !== null && this.canOpenPath(), fileManager };
        }
        /**
         * Verify one path through the composed filesystem and open it on the Host desktop.
         * @param request - path after best-effort Session workspace resolution.
         * @param signal - caller lifetime; abort terminates the native command.
         * @returns confirmation after the native opener accepts the path.
         * @throws RemoteError when the request is invalid, has no verified Host mapping, is cancelled, or the opener fails.
         */
        async openWorkspacePath(request, signal) {
            try {
                const path = await this.verifyDesktopPath(request.path, signal);
                if (request.action === 'reveal')
                    await this.revealPath(path, signal);
                else if (request.application !== undefined)
                    await this.openFileApplication(path, request.application, signal);
                else
                    await this.openPath(path, signal);
                return { opened: true };
            }
            catch (error) {
                if (signal.aborted)
                    throw new RemoteError('gateway/cancelled', 'path open was aborted', {});
                if (error instanceof RemoteError)
                    throw error;
                throw new RemoteError('gateway/internal', 'path open failed', {}, { cause: error });
            }
        }
        /**
         * Query current file handlers on the serving desktop without activating an Agent.
         * @param request - file path in Host filesystem syntax.
         * @param signal - caller lifetime, propagated to filesystem and desktop queries.
         * @returns OS application names, icons, and default selection; empty when desktop opening is unavailable.
         * @throws RemoteError when the path is invalid, the query is cancelled, or native discovery fails.
         */
        async workspacePathApplications(request, signal) {
            if (!this.canOpenPath())
                return [];
            try {
                const path = await this.verifyDesktopPath(request.path, signal);
                return await this.fileApplications(path, signal);
            }
            catch (error) {
                if (signal.aborted)
                    throw new RemoteError('gateway/cancelled', 'application query was aborted', {});
                if (error instanceof RemoteError)
                    throw error;
                throw new RemoteError('gateway/internal', 'file application query failed', {}, { cause: error });
            }
        }
        async verifyDesktopPath(path, signal) {
            if (path.length === 0)
                throw new RemoteError('gateway/bad-request', 'A non-empty file path is required', {});
            signal.throwIfAborted();
            const hostPath = resolve(path);
            const { fs } = this.ctx;
            const mapped = fs.processPathFromHostPath(hostPath);
            if (mapped === undefined || fs.processPath(await fs.resolve(mapped, { signal })) !== hostPath) {
                throw new RemoteError('gateway/bad-request', 'Path has no verified Host path', {});
            }
            signal.throwIfAborted();
            return hostPath;
        }
        /**
         * Rename one Session after explicitly resuming it.
         * @param request - Session identity and proposed title.
         * @returns the accepted title and durable event sequence.
         */
        rename(request) {
            return this.commands.rename(request);
        }
        /**
         * Fork one cold-readable exact event prefix into a new Session. An omitted
         * boundary selects the latest completed-turn prefix; an open cut receives
         * synthetic fork closers.
         * @param request - source Session, optional exact inclusive event boundary, and migration preflight choice.
         * @returns the new Session identity.
         */
        async fork(request) {
            if (request.allowMigration === false && this.ctx.sessions.get(request.sessionId) === undefined) {
                const stored = await this.ctx.get('sessionPersistence')?.stat(request.sessionId);
                if (stored?.formatStatus === 'migration-required' && this.ctx.sessions.get(request.sessionId) === undefined) {
                    throw new RemoteError('session/migration-required', 'Open the source Session to complete migration before forking', {
                        sessionId: request.sessionId,
                    });
                }
            }
            return this.commands.fork(request);
        }
        /**
         * Admit one prompt after explicitly resuming its Session.
         * @param request - Session identity, prompt content, source metadata, and delivery mode.
         * @param signal - caller cancellation before prompt admission begins.
         * @returns acknowledgement that the Agent accepted the prompt.
         */
        async prompt(request, signal) {
            signal.throwIfAborted();
            if (!hasPromptContent(request.content)) {
                throw new RemoteError('gateway/bad-request', 'prompt content must include non-whitespace text or an attachment', {});
            }
            const clientTimeZone = request.clientTimeZone === undefined
                ? undefined
                : canonicalClientTimeZone(request.clientTimeZone);
            if (request.clientTimeZone !== undefined && clientTimeZone === undefined) {
                throw new RemoteError('session/invalid-time-zone', 'clientTimeZone must be UTC or a valid IANA Area/Location name', { value: request.clientTimeZone });
            }
            // The caller is recorded only when it is not the owner, so owner messages keep their exact source.
            const principal = principalOf(this.ctx.invocation);
            const source = {
                kind: 'user',
                rpcId: request.requestId,
                ...(clientTimeZone === undefined ? {} : { clientTimeZone }),
                ...(principal === undefined ? {} : { principal }),
            };
            return this.commands.prompt(request, source);
        }
        /**
         * Read one image from the attachment store after gateway authorization.
         * @param request - image reference and gateway permission scope; Session existence is not checked.
         * @returns the durable attachment reference and base64-encoded bytes.
         */
        attachment(request) {
            return this.commands.attachment(request);
        }
        /**
         * Mutate one still-pending queue occurrence, resuming a cold Agent first.
         * @param request - Session, queue item, and requested mutation.
         * @returns acknowledgement that the queue mutation was applied.
         */
        updateQueue(request) {
            return this.commands.updateQueue(request);
        }
        /**
         * Cancel one active Agent turn without dropping its pending inbox.
         * @param request - Session whose active Agent turn is cancelled.
         * @returns acknowledgement that cancellation was requested.
         */
        cancel(request) {
            return this.commands.cancel(request);
        }
        /**
         * Read one cold-safe, message-aligned Session history page.
         * @deprecated Use `events`; this entry point remains for existing Web callers.
         * @param request - durable address, optional fixed cut, backward cursor, and page budget.
         * @param signal - cancellation for persistence reads.
         * @returns one chronological page with its inclusive cursor; an omitted `throughSeq` selects the observed log end.
         */
        page(request, signal) {
            return this.historyController.events(request, signal);
        }
        /**
         * Read one cold-safe, message-aligned Session history page.
         * @param request - durable address, optional fixed cut, backward cursor, and page budget.
         * @param signal - cancellation for persistence reads.
         * @returns one chronological page with its inclusive cursor.
         */
        events(request, signal) {
            return this.historyController.events(request, signal);
        }
        /**
         * Subscribe to Session events without reading a snapshot.
         * @param request - Session address.
         * @param signal - subscription cancellation.
         * @returns readiness and subsequent events, without a snapshot.
         */
        observeEvent(request, signal) {
            return this.historyController.observeEvent(request, signal);
        }
        /**
         * Subscribe to Session projection changes.
         * @param request - target Session.
         * @param signal - subscription cancellation.
         * @returns readiness followed by one projection change frame per committed event.
         */
        observeProjections(request, signal) {
            return this.historyController.observeProjections(request, signal);
        }
        /**
         * Read passive Session state.
         * @param request - target Session.
         * @param signal - read cancellation.
         * @returns current passive state.
         */
        state(request, signal) {
            return this.stateController.state(request, signal);
        }
        /**
         * Subscribe to complete Session state replacements.
         * @param request - target Session.
         * @param signal - subscription cancellation.
         * @returns complete passive state replacements.
         */
        observeState(request, signal) {
            return this.stateController.observeState(request, signal);
        }
        /**
         * Wait for Session quiescence or pending input with bounded assistant output.
         * @param request - Session, wait bounds and output window.
         * @param signal - caller cancellation.
         * @returns stopping state and bounded assistant output.
         */
        wait(request, signal) {
            return this.stateController.wait(request, signal);
        }
        /**
         * Follow one Session log from its opening or resume cursor.
         * @param request - durable address and last committed sequence already held by the caller.
         * @param signal - cancellation owned by the Remote stream carrier.
         * @returns a complete opening snapshot followed by gap-free durable event
         *   frames and optional cursorless assistant-stream frames.
         */
        follow(request, signal) {
            return this.webController.follow(request, signal);
        }
        /**
         * Read exact projections, returning cached hints only when migration is required.
         * @param request - Session whose current values are required.
         * @param signal - cancellation for the Session read.
         * @returns a sequenced baseline, cached hints when migration is deferred, or null when absent.
         */
        projections(request, signal) {
            return this.historyController.projections(request, signal);
        }
        /**
         * Stream a complete live-control baseline followed by replacement frames.
         * @param signal - cancellation owned by the Remote stream carrier.
         * @returns one complete baseline followed by live replacement frames.
         */
        async *control(signal) {
            for await (const frame of this.controlState.control(signal)) {
                switch (frame.type) {
                    case 'baseline': {
                        const projections = Object.fromEntries(Object.entries(frame.value.projections)
                            .filter(([sessionId]) => this.ctx.remoteAccess.allows({ kind: 'session', id: sessionId })));
                        yield { type: 'baseline', value: { ...frame.value, projections } };
                        break;
                    }
                    case 'projection':
                        if (this.ctx.remoteAccess.allows({ kind: 'session', id: frame.sessionId }))
                            yield frame;
                        break;
                    /* v8 ignore next -- closed control-frame union. */
                    default:
                        assertNever(frame);
                }
            }
        }
    };
})();
export { SessionController };
export default SessionController;
//# sourceMappingURL=index.js.map