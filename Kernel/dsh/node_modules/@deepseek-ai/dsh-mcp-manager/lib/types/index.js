/**
 * MCP server manager (`ctx.mcpManager`): one fleet of `dsh-mcp-client`
 * instances built from two sources. Volatile `Config.servers` holds configured
 * entries, which Settings exposes under the owning Loader entry id with profile
 * inheritance and revision checks; plugins contribute registered entries for
 * their own lifetime through `register()`. Every change to either source
 * reconciles the live instance set: new entries mount, removed or disabled
 * entries unmount, and a changed entry is rebuilt. Connection handling stays
 * entirely in mcp-client; the manager owns entry validation, instance
 * lifecycle, the aggregated status registry, and the `restart`, `probe`, and
 * `watch` operations a management surface calls through Remote.
 *
 * Service package (default-exports the service class). All child instances
 * mount under the manager's fiber, so disposing the manager unwinds every
 * instance and its registered tools.
 *
 * @module @deepseek-ai/dsh-mcp-manager
 */
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
import { Remote, TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import z from '@deepseek-ai/schemastery';
import * as McpClient from '@deepseek-ai/dsh-mcp-client';
import { ReconnectSchema, SERVER_NAME_PATTERN, connectionStatusOf, probeServer, resolveReconnectPolicy } from '@deepseek-ai/dsh-mcp-client';
import { MAX_TIMER_DELAY_MS } from '@deepseek-ai/dsh-timeout';
import { deepEqualJson } from '@deepseek-ai/dsh-util-values';
import { assertTemplateSyntax } from '@deepseek-ai/dsh-mcp-client';
const ServerEntrySchema = z.object({
    transport: z.union(['stdio', 'streamable-http']).required(),
    command: z.string().default(''),
    args: z.array(String).default([]),
    env: z.dict(String).default({}),
    cwd: z.string().default(''),
    url: z.string().default(''),
    headers: z.dict(String).default({}),
    enabled: z.boolean().default(true),
    toolCallTimeoutMs: z.number(),
    startupTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(30_000),
    closeTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS),
    reconnect: ReconnectSchema,
});
/** Fields of one entry that may carry template tokens, with their config paths. */
function templatedStrings(name, entry) {
    const path = `servers.${name}`;
    if (entry.transport === 'stdio') {
        return [
            [entry.command, `${path}.command`],
            ...entry.args.map((arg, index) => [arg, `${path}.args[${String(index)}]`]),
            ...Object.entries(entry.env).map(([key, value]) => [value, `${path}.env.${key}`]),
            [entry.cwd, `${path}.cwd`],
        ];
    }
    return [
        [entry.url, `${path}.url`],
        ...Object.entries(entry.headers).map(([key, value]) => [value, `${path}.headers.${key}`]),
    ];
}
/**
 * Reject entries the manager could not act on: an invalid entry key, a
 * missing or cross-transport field, or a malformed template token. Runs at
 * composition load, at every settings write, and at registration, so a broken
 * entry is refused where it is written instead of silently disabling its server.
 * @param entries - resolved entries keyed by server name.
 */
function validateEntries(entries) {
    for (const [name, entry] of Object.entries(entries)) {
        const path = `servers.${name}`;
        if (!SERVER_NAME_PATTERN.test(name)) {
            throw new Error(`${path}: server name must match ${String(SERVER_NAME_PATTERN)} — rename the entry key`);
        }
        if (entry.transport === 'stdio') {
            if (entry.command === '') {
                throw new Error(`${path}: a stdio server requires "command"`);
            }
            if (entry.url !== '') {
                throw new Error(`${path}: "url" is not a stdio field — remove it or set transport: streamable-http`);
            }
            if (Object.keys(entry.headers).length > 0) {
                throw new Error(`${path}: "headers" is not a stdio field — remove it or set transport: streamable-http`);
            }
        }
        else {
            if (entry.url === '') {
                throw new Error(`${path}: a streamable-http server requires "url"`);
            }
            if (entry.command !== '') {
                throw new Error(`${path}: "command" is not a streamable-http field — remove it or set transport: stdio`);
            }
            if (entry.args.length > 0) {
                throw new Error(`${path}: "args" is not a streamable-http field — remove it or set transport: stdio`);
            }
            if (Object.keys(entry.env).length > 0) {
                throw new Error(`${path}: "env" is not a streamable-http field — remove it or set transport: stdio`);
            }
            if (entry.cwd !== '') {
                throw new Error(`${path}: "cwd" is not a streamable-http field — remove it or set transport: stdio`);
            }
        }
        for (const [value, where] of templatedStrings(name, entry))
            assertTemplateSyntax(value, where);
        // The same cross-field reconnect rules the instance would apply at mount
        // (initialDelayMs <= maxDelayMs, bounds), judged where the entry is written.
        resolveReconnectPolicy(entry.reconnect, `${path}.reconnect`);
    }
}
let McpServerManager = (() => {
    let _classSuper = TypertRemoteService;
    let _instanceExtraInitializers = [];
    let _watch_decorators;
    let _restart_decorators;
    let _probeFromClient_decorators;
    return class McpServerManager extends _classSuper {
        static {
            const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
            _watch_decorators = [Remote({ mode: 'stream' })];
            _restart_decorators = [Remote];
            _probeFromClient_decorators = [Remote('probe')];
            __esDecorate(this, null, _watch_decorators, { kind: "method", name: "watch", static: false, private: false, access: { has: obj => "watch" in obj, get: obj => obj.watch }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _restart_decorators, { kind: "method", name: "restart", static: false, private: false, access: { has: obj => "restart" in obj, get: obj => obj.restart }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _probeFromClient_decorators, { kind: "method", name: "probeFromClient", static: false, private: false, access: { has: obj => "probeFromClient" in obj, get: obj => obj.probeFromClient }, metadata: _metadata }, null, _instanceExtraInitializers);
            if (_metadata) Object.defineProperty(this, Symbol.metadata, { enumerable: true, configurable: true, writable: true, value: _metadata });
        }
        config = __runInitializers(this, _instanceExtraInitializers);
        /** Schema for {@link Config}; Settings exposes its volatile fields. */
        static Config = z.object({
            servers: z.dict(ServerEntrySchema).default({}).volatile(),
        });
        static inject = ['tools'];
        ownerContext;
        instances = new Map();
        statuses = new Map();
        /** Plugin-contributed entries, each owned by its registering plugin's effect. */
        registrations = new Map();
        /** Wake callbacks of active `watch` streams. */
        watchers = new Set();
        /** Aborted at service dispose so `watch` streams end. */
        lifetime = new AbortController();
        /** Settled operation tail serializing every lifecycle operation. */
        tail = Promise.resolve();
        /** Set at service dispose: refuse new operations while queued ones drain. */
        stopped = false;
        /**
         * Validate the resolved server configuration.
         * @param ctx - the manager's plugin context; children mount under its fiber.
         * @param config - resolved manager configuration with live references.
         */
        constructor(ctx, config) {
            super(ctx, 'mcpManager');
            this.config = config;
            this.ownerContext = ctx;
            validateEntries(config.servers.get());
        }
        /**
         * Validate Config updates and reconcile the initial server set. Teardown
         * cancels probes and unmounts every instance, retaining cleanup failures in
         * the readable status registry.
         */
        *[Service.init]() {
            yield async () => {
                this.stop();
                // The tail never rejects: every enqueue swallows its own settlement.
                await this.tail;
                for (const [name, record] of [...this.instances])
                    await this.unmount(name, record);
            };
            const owner = this.ctx.fiber;
            const registrations = this.registrations;
            this.ctx.on('internal/config', (fiber, next) => {
                const raw = next();
                if (fiber !== owner)
                    return raw;
                const servers = McpServerManager.Config(raw).servers.get();
                validateEntries(servers);
                const taken = Object.keys(servers).find(name => registrations.has(name));
                if (taken !== undefined)
                    throw new Error(`servers.${taken}: a plugin already registers this server name — rename the entry key`);
                return raw;
            });
            this.ctx.on('internal/plugin', (fiber) => {
                if (fiber === owner && fiber.uid === null)
                    this.stop();
            }, { global: true });
            this.ctx.on('loader/volatile-update', () => { void this.entriesChanged('config reconcile'); });
            // The service is visible to dependents only once this fiber is ACTIVE, so
            // the first reconcile waits for activation instead of racing it.
            void this.ctx.fiber.await().then(() => {
                void this.schedule(() => this.reconcileOnce(), 'initial reconcile');
            }, 
            /* v8 ignore next 2 -- init is this fiber's whole startup and this registration is its
               last statement, so a rejected activation cannot reach a registered continuation */
            () => undefined);
        }
        /**
         * Read configured and registered servers and retained runtime records without mounting or probing.
         * @returns configured entries in document order, then registered entries, then retained records for removed entries.
         */
        list() {
            const entries = this.entries();
            return [...new Set([...Object.keys(entries), ...this.instances.keys()])].map(name => ({
                name,
                configured: Object.hasOwn(entries, name),
                status: this.statuses.get(name)
                    ?? (entries[name]?.enabled ? { state: 'connecting', attempt: 0 } : { state: 'disabled' }),
            }));
        }
        /**
         * Contribute one server for the calling plugin's lifetime. A registered
         * entry is enabled while registered; its removal awaits the instance's
         * unmount, so plugin disposal withdraws the server's tools before settling.
         * @param name - server key and tool namespace, unique across configured and registered servers.
         * @param entry - transport, endpoint, and budgets, validated like a configured entry.
         * @returns an idempotent disposer that removes the registration.
         * @throws when the name is taken, the entry is invalid, or the manager is disposed.
         */
        register(name, entry) {
            const resolved = ServerEntrySchema({ ...entry, enabled: true });
            validateEntries({ [name]: resolved });
            const dispose = this.ctx.effect(() => {
                if (this.stopped)
                    throw new Error('mcp-manager: the manager is disposed');
                if (Object.hasOwn(this.config.servers.get(), name) || this.registrations.has(name)) {
                    throw new Error(`mcp-manager: server "${name}" is already configured or registered`);
                }
                this.registrations.set(name, resolved);
                void this.entriesChanged(`registration of "${name}"`);
                return () => {
                    this.registrations.delete(name);
                    // A stopping manager unmounts every instance itself.
                    if (!this.stopped)
                        return this.entriesChanged(`removal of "${name}"`);
                };
            }, 'mcpManager.register()');
            return () => void dispose();
        }
        /**
         * Stream server views for a management surface: the current list on
         * subscription, then the complete list again after each status or entry change.
         * @param signal - subscription lifetime; aborting ends only this stream.
         * @returns coalesced server lists until the subscriber or the manager stops.
         */
        async *watch(signal) {
            const ended = AbortSignal.any([signal, this.lifetime.signal]);
            // Read through a call: the flag changes while the stream awaits.
            const aborted = () => ended.aborted;
            let wake = Promise.withResolvers();
            let dirty = true;
            const notify = () => {
                dirty = true;
                wake.resolve();
            };
            this.watchers.add(notify);
            ended.addEventListener('abort', notify, { once: true });
            try {
                while (!aborted()) {
                    if (!dirty)
                        await wake.promise;
                    if (aborted())
                        break;
                    dirty = false;
                    wake = Promise.withResolvers();
                    yield this.list();
                }
            }
            finally {
                this.watchers.delete(notify);
                ended.removeEventListener('abort', notify);
            }
        }
        /**
         * Rebuild one server from its current entry: dispose the live instance (if
         * any) and mount a fresh one with a fresh reconnect budget. Incomplete
         * cleanup blocks replacement until the Host restarts.
         * @param name - the configured or registered server key.
         * @throws when the manager is disposed, no such entry exists, or cleanup is incomplete.
         */
        async restart(name) {
            await this.enqueue(async () => {
                if (this.entries()[name] === undefined) {
                    throw new Error(`mcp-manager: unknown server "${name}" — it is neither configured nor registered`);
                }
                await this.remount(name);
            });
        }
        /**
         * Probe one server for a management surface; see {@link McpServerManager.probe}.
         * @param name - the configured or registered server key.
         * @param signal - caller cancellation.
         * @returns the stage-classified probe outcome and independent cleanup result.
         */
        probeFromClient(name, signal) {
            return this.probe(name, { signal });
        }
        /**
         * Probe one configured server without touching its live instance: resolve
         * the entry's templates, connect, list tools, close. A disabled entry can be
         * probed — that is how a management surface tests it before enabling.
         * Configuration changes and manager disposal cancel the probe.
         * @param name - the configured or registered server key.
         * @param options - probe budget, listing caps, and abort signal.
         * @returns the stage-classified probe outcome and independent cleanup result.
         * @throws when the manager is disposed, the entry is absent or changing, cleanup is
         * incomplete, options are invalid, or a required credentials provider is missing.
         */
        async probe(name, options) {
            if (this.stopped)
                throw new Error('mcp-manager: the manager is disposed');
            const entry = this.entries()[name];
            if (entry === undefined)
                throw new Error(`mcp-manager: unknown server "${name}"`);
            let record = this.instances.get(name);
            if (record === undefined) {
                record = { entrySnapshot: entry, probes: new Map() };
                this.instances.set(name, record);
            }
            if (record.retired || !deepEqualJson(entry, record.entrySnapshot))
                throw new Error(`mcp-manager: configuration for "${name}" is changing`);
            if (record.failure !== undefined)
                throw new Error(`mcp-manager: cleanup for "${name}" is incomplete; restart the Host`);
            const lifetime = new AbortController();
            const owner = record;
            const signal = options?.signal === undefined ? lifetime.signal : AbortSignal.any([lifetime.signal, options.signal]);
            const done = probeServer(this.ownerContext, this.toClientConfig(name, entry), { ...options, signal }).then((result) => {
                const closure = { closeConfirmed: result.closeConfirmed,
                    ...result.cleanupError === undefined ? {} : { cleanupError: result.cleanupError } };
                if (!closure.closeConfirmed || closure.cleanupError !== undefined) {
                    this.setStatus(name, { state: 'failed', reason: 'cleanup', result: this.retainCleanupFailure(owner, closure) });
                }
                return result;
            }).finally(() => { owner.probes.delete(lifetime); });
            owner.probes.set(lifetime, done);
            return done;
        }
        retire(record) {
            record.retired = true;
            record.detachStatus?.();
            for (const lifetime of record.probes.keys())
                lifetime.abort();
        }
        stop() {
            this.stopped = true;
            this.lifetime.abort();
            for (const record of this.instances.values())
                this.retire(record);
        }
        /** Configured entries in document order, followed by registered entries. */
        entries() {
            return { ...this.config.servers.get(), ...Object.fromEntries(this.registrations) };
        }
        /** Retire records whose entry changed or disappeared, then queue reconciliation. */
        entriesChanged(what) {
            const entries = this.entries();
            for (const [name, record] of this.instances) {
                if (!deepEqualJson(record.entrySnapshot, entries[name]))
                    this.retire(record);
            }
            this.changed();
            return this.schedule(() => this.reconcileOnce(), what);
        }
        /** Wake every `watch` stream. */
        changed() {
            for (const notify of this.watchers)
                notify();
        }
        /** Queue one lifecycle operation on the serialized tail and contain its failure. */
        schedule(op, what) {
            return this.enqueue(op).catch((error) => {
                this.ctx.logger.error(`mcp-manager: ${what} failed`);
                this.ctx.logger.error(error);
            });
        }
        /** Serialize one operation behind every queued predecessor. */
        enqueue(op) {
            const run = this.tail.then(async () => {
                if (this.stopped) {
                    throw new Error('mcp-manager: the manager is disposed');
                }
                return op();
            });
            this.tail = run.then(() => undefined, () => undefined);
            return run;
        }
        /** Reconcile the live instance set against the current entries. */
        async reconcileOnce() {
            const servers = this.entries();
            for (const [name, record] of this.instances) {
                const entry = servers[name];
                if (!record.retired && entry !== undefined && deepEqualJson(entry, record.entrySnapshot))
                    continue;
                await this.unmount(name, record);
            }
            for (const name of this.statuses.keys()) {
                if (servers[name] === undefined && !this.instances.has(name))
                    this.statuses.delete(name);
            }
            this.changed();
            for (const [name, entry] of Object.entries(servers)) {
                const record = this.instances.get(name);
                if (record?.retired || record?.failure !== undefined)
                    continue;
                if (!entry.enabled) {
                    this.setStatus(name, { state: 'disabled' });
                    continue;
                }
                if (record?.handle !== undefined)
                    continue;
                this.mount(name, entry, record);
            }
        }
        async remount(name) {
            const record = this.instances.get(name);
            if (record !== undefined && !await this.unmount(name, record)) {
                throw new Error(`mcp-manager: cleanup for "${name}" is incomplete; restart the Host`);
            }
            const entry = this.entries()[name];
            if (entry === undefined || this.stopped)
                return;
            if (!entry.enabled) {
                this.setStatus(name, { state: 'disabled' });
                return;
            }
            this.mount(name, entry);
        }
        mount(name, entry, existing) {
            if (this.stopped || !deepEqualJson(this.entries()[name], entry))
                return;
            const record = existing ?? { entrySnapshot: entry, probes: new Map() };
            this.instances.set(name, record);
            this.setStatus(name, { state: 'connecting', attempt: 0 });
            const handle = record.handle = this.ownerContext.plugin(McpClient, this.toClientConfig(name, entry));
            void handle.await().then(() => {
                if (record.retired)
                    return;
                const cell = connectionStatusOf(handle);
                /* v8 ignore next -- successful MCP activation installs its status cell before becoming ready */
                if (cell === undefined)
                    throw new Error(`mcp-manager: "${name}" activated without connection status`);
                const publish = () => {
                    if (record.retired)
                        return;
                    const status = cell.current();
                    if (status.state === 'failed' && status.reason === 'unclosed-generation') {
                        const hadFailure = record.failure !== undefined;
                        const result = this.retainCleanupFailure(record, cell.closure());
                        this.setStatus(name, hadFailure ? { state: 'failed', reason: 'cleanup', result } : status);
                    }
                    else if (record.failure === undefined)
                        this.setStatus(name, status);
                };
                record.detachStatus = cell.onChange(publish);
                publish();
            }).catch((error) => {
                if (record.retired || this.stopped)
                    return;
                void this.schedule(async () => {
                    if (record.retired)
                        return;
                    if (await this.unmount(name, record))
                        this.setStatus(name, { state: 'failed', reason: 'load', lastError: String(error) });
                }, `failed startup of "${name}"`);
            });
        }
        async unmount(name, record) {
            this.retire(record);
            await Promise.allSettled([...record.probes.values()]);
            if (record.handle !== undefined) {
                await record.handle.dispose();
                const result = connectionStatusOf(record.handle)?.closure();
                if (result !== undefined && (!result.closeConfirmed || result.cleanupError !== undefined))
                    this.retainCleanupFailure(record, result);
            }
            if (record.failure !== undefined) {
                this.setStatus(name, { state: 'failed', reason: 'cleanup', result: record.failure });
                return false;
            }
            this.instances.delete(name);
            return true;
        }
        /** Retain the first cleanup diagnostic and every unconfirmed transport closure. */
        retainCleanupFailure(record, result) {
            return record.failure = { ...result, ...record.failure,
                closeConfirmed: result.closeConfirmed && record.failure?.closeConfirmed !== false };
        }
        /**
         * Publish changed statuses for configured or registered entries and retained runtime records.
         * Removed entries with incomplete cleanup remain observable; disposed managers
         * retain the final status without emitting events.
         */
        setStatus(name, status) {
            // A lifecycle operation that entered before disposal may still be
            // finishing while the service is torn down; nothing may emit for a
            // manager that is no longer live.
            if (this.entries()[name] === undefined && !this.instances.has(name))
                return;
            const previous = this.statuses.get(name);
            if (previous !== undefined && deepEqualJson(previous, status))
                return;
            this.statuses.set(name, status);
            if (this.stopped)
                return;
            this.changed();
            void this.ctx.parallel('mcp-manager/status', name, status).catch((error) => {
                this.ctx.logger.error(`mcp-manager: a mcp-manager/status listener failed for "${name}"`);
                this.ctx.logger.error(error);
            });
        }
        /**
         * Resolve client defaults while retaining template strings for runtime
         * expansion. Non-strict startup exposes connection failures through status;
         * the manager schema supplies its startup budget.
         */
        toClientConfig(name, entry) {
            const shared = {
                serverName: name,
                failOnStartupError: false,
                expandTemplates: true,
                ...entry.closeTimeoutMs === undefined ? {} : { closeTimeoutMs: entry.closeTimeoutMs },
                ...entry.toolCallTimeoutMs === undefined ? {} : { toolCallTimeoutMs: entry.toolCallTimeoutMs },
                startupTimeoutMs: entry.startupTimeoutMs,
                reconnect: entry.reconnect,
            };
            // Resolving through the client's own schema fills its defaults here (no
            // restatement) and returns a real Config with no type assertion; the
            // write-time validation above keeps this resolution from ever refusing.
            if (entry.transport === 'stdio') {
                return McpClient.Config({
                    transport: 'stdio',
                    command: entry.command,
                    args: [...entry.args],
                    env: { ...entry.env },
                    cwd: entry.cwd,
                    ...shared,
                });
            }
            return McpClient.Config({
                transport: 'streamable-http',
                url: entry.url,
                headers: { ...entry.headers },
                ...shared,
            });
        }
    };
})();
/**
 * The manager service. Reconciliation and `restart` serialize instance
 * replacement. Probes run independently and are cancelled before replacement
 * or disposal waits for their cleanup.
 */
export default McpServerManager;
//# sourceMappingURL=index.js.map