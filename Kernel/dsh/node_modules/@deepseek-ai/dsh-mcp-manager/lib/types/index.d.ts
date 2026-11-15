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
import { Context, Service } from '@deepseek-ai/cordis';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { Volatile } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { McpProbeOptions, McpProbeResult, ReconnectConfig } from '@deepseek-ai/dsh-mcp-client';
import type { McpManagedServerView } from './types.ts';
export type { McpManagedServerStatus, McpManagedServerView } from './types.ts';
/**
 * One managed server entry. Deliberately flat rather than a per-transport
 * union: {@link validateEntries} owns the per-transport field rules, and its
 * write-time refusals name the offending field where a union's shape error
 * would not.
 */
export interface McpManagedServerEntry {
    /** Which transport the instance uses. */
    transport: 'stdio' | 'streamable-http';
    /** stdio: executable used to start the server; `''` while unset. */
    command: string;
    /** stdio: arguments passed without shell interpolation. */
    args: string[];
    /** stdio: extra child environment; values may use `${credential:…}` templates. */
    env: Record<string, string>;
    /** stdio: working directory for the child process. */
    cwd: string;
    /** streamable-http: MCP endpoint URL; `''` while unset. */
    url: string;
    /** streamable-http: extra request headers; values may use templates. */
    headers: Record<string, string>;
    /** Durable intent: whether the entry asks for a running instance. */
    enabled: boolean;
    /** Per-tool-call timeout override in milliseconds; omission uses the mcp-client default. */
    toolCallTimeoutMs?: number;
    /** Per-attempt startup budget in milliseconds; the manager schema defaults to 30000. */
    startupTimeoutMs: number;
    /** Cleanup budget override in milliseconds; omission uses the mcp-client default. */
    closeTimeoutMs?: number;
    /** Reconnect policy; the schema fills the mcp-client defaults. */
    reconnect: ReconnectConfig;
}
/** Manager configuration with live server entries. */
export interface Config {
    /** Managed servers keyed by `serverName`. */
    servers: Volatile<Record<string, McpManagedServerEntry>>;
}
type EntryInput = Omit<McpManagedServerEntry, 'command' | 'url' | 'args' | 'env' | 'cwd' | 'headers' | 'enabled' | 'reconnect' | 'startupTimeoutMs'> & Partial<Pick<McpManagedServerEntry, 'command' | 'url' | 'args' | 'env' | 'cwd' | 'headers' | 'enabled' | 'reconnect' | 'startupTimeoutMs'>>;
interface ConfigInput {
    servers?: Record<string, EntryInput>;
}
/** One server a plugin contributes for its own lifetime; it is enabled while registered. */
export type McpServerRegistration = Omit<EntryInput, 'enabled'>;
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** The MCP server manager. */
        mcpManager: McpServerManager;
    }
}
/**
 * The manager service. Reconciliation and `restart` serialize instance
 * replacement. Probes run independently and are cancelled before replacement
 * or disposal waits for their cleanup.
 */
export default class McpServerManager extends TypertRemoteService {
    config: Config;
    /** Schema for {@link Config}; Settings exposes its volatile fields. */
    static Config: z<ConfigInput, Config>;
    static inject: string[];
    private readonly ownerContext;
    private readonly instances;
    private readonly statuses;
    /** Plugin-contributed entries, each owned by its registering plugin's effect. */
    private readonly registrations;
    /** Wake callbacks of active `watch` streams. */
    private readonly watchers;
    /** Aborted at service dispose so `watch` streams end. */
    private readonly lifetime;
    /** Settled operation tail serializing every lifecycle operation. */
    private tail;
    /** Set at service dispose: refuse new operations while queued ones drain. */
    private stopped;
    /**
     * Validate the resolved server configuration.
     * @param ctx - the manager's plugin context; children mount under its fiber.
     * @param config - resolved manager configuration with live references.
     */
    constructor(ctx: Context, config: Config);
    /**
     * Validate Config updates and reconcile the initial server set. Teardown
     * cancels probes and unmounts every instance, retaining cleanup failures in
     * the readable status registry.
     */
    [Service.init](): Generator<() => Promise<void> | void, void, void>;
    /**
     * Read configured and registered servers and retained runtime records without mounting or probing.
     * @returns configured entries in document order, then registered entries, then retained records for removed entries.
     */
    list(): McpManagedServerView[];
    /**
     * Contribute one server for the calling plugin's lifetime. A registered
     * entry is enabled while registered; its removal awaits the instance's
     * unmount, so plugin disposal withdraws the server's tools before settling.
     * @param name - server key and tool namespace, unique across configured and registered servers.
     * @param entry - transport, endpoint, and budgets, validated like a configured entry.
     * @returns an idempotent disposer that removes the registration.
     * @throws when the name is taken, the entry is invalid, or the manager is disposed.
     */
    register(name: string, entry: McpServerRegistration): () => void;
    /**
     * Stream server views for a management surface: the current list on
     * subscription, then the complete list again after each status or entry change.
     * @param signal - subscription lifetime; aborting ends only this stream.
     * @returns coalesced server lists until the subscriber or the manager stops.
     */
    watch(signal: AbortSignal): AsyncIterable<McpManagedServerView[]>;
    /**
     * Rebuild one server from its current entry: dispose the live instance (if
     * any) and mount a fresh one with a fresh reconnect budget. Incomplete
     * cleanup blocks replacement until the Host restarts.
     * @param name - the configured or registered server key.
     * @throws when the manager is disposed, no such entry exists, or cleanup is incomplete.
     */
    restart(name: string): Promise<void>;
    /**
     * Probe one server for a management surface; see {@link McpServerManager.probe}.
     * @param name - the configured or registered server key.
     * @param signal - caller cancellation.
     * @returns the stage-classified probe outcome and independent cleanup result.
     */
    probeFromClient(name: string, signal: AbortSignal): Promise<McpProbeResult>;
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
    probe(name: string, options?: McpProbeOptions): Promise<McpProbeResult>;
    private retire;
    private stop;
    /** Configured entries in document order, followed by registered entries. */
    private entries;
    /** Retire records whose entry changed or disappeared, then queue reconciliation. */
    private entriesChanged;
    /** Wake every `watch` stream. */
    private changed;
    /** Queue one lifecycle operation on the serialized tail and contain its failure. */
    private schedule;
    /** Serialize one operation behind every queued predecessor. */
    private enqueue;
    /** Reconcile the live instance set against the current entries. */
    private reconcileOnce;
    private remount;
    private mount;
    private unmount;
    /** Retain the first cleanup diagnostic and every unconfirmed transport closure. */
    private retainCleanupFailure;
    /**
     * Publish changed statuses for configured or registered entries and retained runtime records.
     * Removed entries with incomplete cleanup remain observable; disposed managers
     * retain the final status without emitting events.
     */
    private setStatus;
    /**
     * Resolve client defaults while retaining template strings for runtime
     * expansion. Non-strict startup exposes connection failures through status;
     * the manager schema supplies its startup budget.
     */
    private toClientConfig;
}
//# sourceMappingURL=index.d.ts.map