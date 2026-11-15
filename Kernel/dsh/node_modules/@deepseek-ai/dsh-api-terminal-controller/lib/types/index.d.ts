import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { SessionId } from '@deepseek-ai/dsh-session';
import { TypertRemoteService, type RemoteStream } from '@deepseek-ai/dsh-typert-protocol';
import { type AgentTerminal, type AgentTerminalRequest } from './agent.ts';
import type { TerminalShell, TerminalInput, TerminalCreateRequest, TerminalEnvironment, TerminalFrame, TerminalRetentionFrame, TerminalScreenPage, TerminalScreenRequest, WebTerminalId, WebTerminalInfo } from './types.ts';
export type * from './types.ts';
export { AgentTerminalError } from './agent.ts';
export type { AgentTerminal, AgentTerminalErrorCode, AgentTerminalRequest } from './agent.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** Interactive user terminals, separate from the Agent terminal tool registry. */
        terminalController: TerminalController;
    }
}
/** Deployment limits and an optional shell profile. */
export interface Config {
    /** Explicit shell profile; omission uses the execution environment's default shell. */
    readonly shell?: {
        /** Executable path or PATH name, verified by the subprocess provider. */
        path: string;
        /** User-visible profile name. */
        name: string;
        /** Arguments passed to the interactive shell. */
        args: string[];
    } | undefined;
    /** Executable names or paths checked for the new-terminal shell selector. */
    readonly shellCandidates: string[];
    /** Maximum retained terminals and pending allocations per Session. */
    readonly maxTerminals: number;
    /**
     * Maximum running Agent-controlled terminals and pending Agent allocations per Session; `maxTerminals` still bounds
     * every terminal, including exited ones and failed allocations awaiting cleanup.
     */
    readonly maxAgentTerminals: number;
    /** Maximum terminal width in columns. */
    readonly maxCols: number;
    /** Maximum terminal height in rows. */
    readonly maxRows: number;
    /** Screen history rows retained for reconnecting clients. */
    readonly scrollback: number;
    /** Maximum queued UTF-8 frame bytes per output follower and in the Client input buffer. */
    readonly maxBufferedBytes: number;
    /** Maximum raw UTF-8 bytes per input command and across unacknowledged Client input. */
    readonly maxInputBytes: number;
    /** Provider process-termination grace period in milliseconds. */
    readonly disposeGraceMs: number;
    /** Continuous confirmed idle time without window holds before reclamation; zero disables reclamation. */
    readonly unattendedTimeoutMs: number;
    /** Interval between unattended shell and process observations. */
    readonly activityPollIntervalMs: number;
    /** Delay before retrying failed owned terminal cleanup. */
    readonly cleanupRetryMs: number;
}
/** Typed Remote control of transient Session-owned terminal processes. */
export declare class TerminalController extends TypertRemoteService {
    private readonly config;
    static inject: string[];
    static Config: z<Config>;
    private readonly owners;
    private readonly rosters;
    private readonly lifetime;
    /**
     * @param ctx - Host context carrying typed Remote and execution providers.
     * @param config - validated terminal limits and optional shell profile.
     */
    constructor(ctx: Context, config: Config);
    /**
     * Read the Session's current directory and terminal limits without filesystem validation or shell lookup.
     * @param agent - Session owner supplied by the Gateway.
     * @param signal - request cancellation.
     * @returns the logged current directory and terminal limits; retained terminals keep their own process directories.
     */
    environment(agent: Agent, signal: AbortSignal): TerminalEnvironment;
    /**
     * Discover installed shells in the Session's execution environment.
     * @param agent - Session owner supplied by the Gateway.
     * @param signal - request cancellation.
     * @returns verified profiles, with the configured or system default first.
     */
    shells(agent: Agent, signal: AbortSignal): Promise<TerminalShell[]>;
    /**
     * List retained terminals without resolving or activating an Agent.
     * @param sessionId - displayed Session identity, including offline history.
     * @returns terminals retained for this Host lifetime.
     */
    list(sessionId: SessionId): WebTerminalInfo[];
    /**
     * Allocate a user shell once for a caller-generated identity, without Agent sandbox or approval restrictions.
     * @param agent - Session owner supplied by the Gateway.
     * @param request - initial dimensions and idempotency identity.
     * @param signal - allocation cancellation; committed terminals survive disconnection.
     * @returns the existing or newly committed terminal.
     */
    create(agent: Agent, request: TerminalCreateRequest, signal: AbortSignal): Promise<WebTerminalInfo>;
    /**
     * Open a terminal for the Session's Agent under a caller-resolved sandbox policy; it appears in every
     * window that watches the Session and passes to the person on the person's first typed input.
     * @param agent - Session owner whose Agent requests the terminal.
     * @param request - title, directory, dimensions, sandbox policy and shell preference.
     * @param signal - allocation cancellation; a committed terminal outlives it.
     * @returns the Agent's handle for the committed terminal.
     * @throws RemoteError `terminal/limit-reached` when the Session or Agent terminal limit is reached.
     * @throws Error when the title is blank or longer than 120 characters.
     */
    openForAgent(agent: Agent, request: AgentTerminalRequest, signal: AbortSignal): Promise<AgentTerminal>;
    /**
     * Render a page of any terminal in the Agent's Session, including terminals the person opened.
     * @param agent - Session owner.
     * @param id - terminal identity.
     * @param request - page position and byte bound.
     * @returns rendered screen text, including output received before the call.
     * @throws RemoteError `terminal/unavailable` when the terminal is missing or closes before the screen is read.
     */
    readScreen(agent: Agent, id: WebTerminalId, request: TerminalScreenRequest): Promise<TerminalScreenPage>;
    /**
     * Observe whether a Session terminal's shell is running a command.
     * @param agent - Session owner.
     * @param id - terminal identity.
     * @returns idle, busy, or unknown for unsupported shells and sandboxed launches.
     */
    inspectActivity(agent: Agent, id: WebTerminalId): Promise<'idle' | 'busy' | 'unknown'>;
    /**
     * Stream the Session's retained terminals as whole sets, without resolving or activating an Agent.
     * @param sessionId - watched Session identity.
     * @param signal - stream cancellation.
     * @returns the current set, then a new set after creation, removal or any published state change.
     */
    watch(sessionId: SessionId, signal: AbortSignal): AsyncIterable<WebTerminalInfo[]>;
    /**
     * Retain an existing terminal for a window without activating its Agent or taking input control.
     * @param sessionId - owning Session identity, including an inactive saved layout.
     * @param id - retained Host terminal identity.
     * @param signal - physical Remote stream cancellation.
     * @returns a hold acknowledgement followed by an open lifetime stream.
     */
    retain(sessionId: SessionId, id: WebTerminalId, signal: AbortSignal): AsyncIterable<TerminalRetentionFrame>;
    /**
     * Attach terminal input and output to one physical Remote stream.
     * @param agent - Session owner supplied by the Gateway.
     * @param id - terminal identity.
     * @param signal - attachment cancellation; the process remains alive and input is never replayed.
     * @returns screen recovery, output, writable state and provider-acceptance acknowledgements.
     */
    attach(agent: Agent, id: WebTerminalId, signal: AbortSignal): RemoteStream<TerminalFrame, TerminalInput>;
    /**
     * Rename a terminal without changing its shell.
     * @param agent - Session owner supplied by the Gateway.
     * @param id - terminal identity.
     * @param title - nonempty display title, at most 120 characters.
     */
    rename(agent: Agent, id: WebTerminalId, title: string): void;
    /**
     * Close an identity to future creation and kill its process range; repeated closes succeed.
     * @param agent - Session owner supplied by the Gateway.
     * @param id - terminal identity.
     * @returns after provider cleanup succeeds. A failure retains the terminal for retry.
     */
    close(agent: Agent, id: WebTerminalId): Promise<void>;
    private owner;
    private disposeOwner;
    private changed;
    private requireCapacity;
    private allocate;
    private userLaunch;
    private agentLaunch;
    /**
     * Delete a confined Agent shell's history file in its execution environment after the shell ends.
     * @param subprocess - the execution environment that ran the shell.
     * @param path - history file the shell was given.
     * @returns after deletion; a failure is logged because the file only holds the ended shell's own history.
     */
    private removeHistory;
    private agentShell;
    private confine;
    private terminal;
    private requireOpen;
    private dimensions;
    private execution;
    private spawn;
}
/** Browser terminal service plugin. */
export default TerminalController;
//# sourceMappingURL=index.d.ts.map