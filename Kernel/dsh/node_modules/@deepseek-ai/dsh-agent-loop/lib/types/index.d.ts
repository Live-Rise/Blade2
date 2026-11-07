/**
 * Concrete agent-loop plugin: creates scoped ReactLoopAgents, publishes them
 * through the agent/session registries, and owns their ordered teardown.
 *
 * @module @deepseek-ai/dsh-agent-loop
 */
import type { Volatile } from '@deepseek-ai/cosmokit';
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Agent, AgentFactory, AgentHandle, AgentOptions, CreateAgentOptions, ResumeAgentOptions } from '@deepseek-ai/dsh-agent';
import type { SessionHeader, SessionId } from '@deepseek-ai/dsh-session';
import { DEFAULT_MAX_PARALLEL_TOOL_CALLS } from './constants.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        agentLoop: AgentLoop;
        /**
         * Launcher-owned exact session identities for configured agents, keyed by
         * the agent's config `id` and set with `ctx.provide()` before any Loader
         * entry mounts (see {@link CONFIGURED_AGENT_IDENTITIES_KEY}). A launcher
         * owns identity because only it knows whether the session already exists,
         * while the `cordis.yml` row keeps the model route as ordinary patchable
         * config. An entry with no matching key keeps its configured identity.
         */
        configuredAgentIdentities?: ConfiguredAgentIdentities;
    }
    interface Events {
        /**
         * A declarative agent entry failed before it could publish a live agent.
         * Consumers that buffer work for the configured identity use this
         * transient signal to reject that work instead of waiting forever. Normal
         * factory teardown suppresses failures from the cancelled startup attempt.
         * @param payload.sessionId - exact shared agent/session identity that failed startup.
         * @param payload.error - persistence, setup, or publication failure.
         * @mode emit
         */
        'agent-loop/config-start-failed'(payload: {
            sessionId: SessionId;
            error: unknown;
        }): void;
    }
}
export { DEFAULT_MAX_PARALLEL_TOOL_CALLS };
/**
 * One launcher-selected session identity for a configured agent. `resume`
 * distinguishes rehydrating existing persisted history from creating the
 * session fresh under that exact id, which the two config keys express as
 * `resumeSessionId` and `sessionId`.
 */
export interface LauncherAgentIdentity {
    /** Exact session id to create fresh or resume. */
    id: SessionId;
    /** Resume existing persisted history instead of creating the session fresh. */
    resume: boolean;
}
/** Launcher-selected identities keyed by the configured agent's `id`. */
export interface ConfiguredAgentIdentities extends Readonly<Record<string, LauncherAgentIdentity>> {
}
/**
 * Context key a launcher sets before any Loader entry mounts
 * (`ctx.provide(CONFIGURED_AGENT_IDENTITIES_KEY, identities)`) to fix
 * configured agents' session identities without a config key, so an overlay
 * repointing the row's model route cannot drop them.
 */
export declare const CONFIGURED_AGENT_IDENTITIES_KEY = "configuredAgentIdentities";
/** Agent-loop plugin configuration. */
export interface Config {
    /**
     * Maximum parallel-safe calls in flight per agent step. `1` is serial;
     * omission defaults to {@link DEFAULT_MAX_PARALLEL_TOOL_CALLS}.
     */
    maxParallelToolCalls: Volatile<number>;
    /** Agents created or resumed at plugin startup; a resumed agent records its configured route as the Session's model selection. */
    agents: (AgentOptions & {
        /** Stable config label used in logs and as the fresh combined-id prefix. */
        id: string;
        /** Optional stable identity; remounts resume its materialized history, while first use creates it fresh. */
        sessionId?: SessionId;
        /** Optional workspace for a fresh session. */
        cwd?: string;
        /** Persisted session to resume instead of creating a fresh session. */
        resumeSessionId?: SessionId;
    })[];
}
/** Concrete agent factory and driver service. */
export declare class AgentLoop extends Service implements AgentFactory {
    static inject: string[];
    /** Runtime schema for declarative agents. */
    static Config: z<{
        agents?: Config['agents'];
        maxParallelToolCalls?: number;
    }, Config>;
    /** Validated configuration owned by the agent-loop service. */
    readonly config: Config;
    private readonly ownership;
    /** Plain holder prevents Cordis from re-tracing the factory's dependency context through a caller shadow. */
    private readonly runtime;
    private modelProjectionHandle;
    private inboxProjectionHandle;
    constructor(ctx: Context, config: Config);
    protected [Service.init](): Promise<void>;
    private applyConfiguredRoute;
    /** Report a contained declarative-start failure to identity-bound consumers. */
    private reportConfiguredStartupFailure;
    /** Restore a materialized exact config identity on remount, or create it on first use. */
    private restoreOrCreateConfigured;
    /** Wait for a draining same-id lifecycle to finish registry teardown. */
    private waitForDrainingConfiguredIdentity;
    /**
     * Own initialization and teardown from an acquired Session to a published Agent.
     * Registers factory teardown and caller cancellation before constructing the scope.
     * Failed initialization drains cleanup while retaining stored events and inbox input.
     * @param ownerCtx - caller context that owns the Agent lifecycle.
     * @param options - acquired Session/write handle, loop options, start source, parent Agent, setup, and cancellation.
     * @returns the initialized Agent and its shared disposer.
     */
    private initializeAgent;
    /**
     * Create an agent and session under one caller-supplied identity, owned by
     * the accessing fiber. Constructor-driven config calls mint a fresh combined
     * id before entering this boundary. When a persistence backend is mounted,
     * the session's durable identity and any seed are stored before publication.
     * @param id - shared agent/session identity.
     * @param options - concrete loop options.
     * @param meta - optional fresh-session workspace metadata.
     * @returns the published running agent.
     */
    create(id: SessionId, options?: AgentOptions, meta?: Pick<SessionHeader, 'cwd'>): Promise<Agent>;
    /**
     * Take a fresh session's write ownership when persistence is mounted.
     * The constructor seed is stored before Session store entry. Setup then
     * appends through the live event feed; creation failure retains any events
     * already written to the handle.
     * @param session - the unpublished session to store.
     * @param signal - optional cancellation forwarded to the backend create.
     * @returns the owned handle and stored cursor, or `undefined` without a backend.
     */
    private createStoredSession;
    /**
     * Durably store the session events appended since the last stored cursor.
     * @param stored - the session's owned handle and stored cursor, if any.
     * @param session - the unpublished session whose suffix is stored.
     */
    private appendUnstoredSuffix;
    /**
     * Create an owned agent on a caller-supplied session id.
     * @param ownerCtx - caller context that structurally owns the lifecycle.
     * @param options - identities, optional live parent, session seed/metadata, loop options, setup, and cancellation.
     * @returns the published handle.
     */
    createAgent(ownerCtx: Context, options: CreateAgentOptions): Promise<AgentHandle>;
    /**
     * Resume an owned agent from the configured persistence service.
     * @param ownerCtx - caller context that owns load, setup, and the live lifecycle.
     * @param options - persisted identity, optional live parent, loop options, setup, and cancellation.
     * @returns the published handle.
     */
    resume(ownerCtx: Context, options: ResumeAgentOptions): Promise<AgentHandle>;
    /** Resume through an explicit persistence handle used by the deferred config path. */
    private resumeWith;
}
export default AgentLoop;
//# sourceMappingURL=index.d.ts.map