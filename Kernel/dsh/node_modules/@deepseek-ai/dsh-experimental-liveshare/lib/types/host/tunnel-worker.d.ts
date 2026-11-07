/** Owner of the single tunnel worker process used by one DSH web process. */
import { type TunnelWorkerCommand, type TunnelWorkerEvent } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
/** A Node executable plus argv used to start the worker in source or built mode. */
export interface TunnelProcessCommand {
    readonly executable: string;
    readonly args: readonly string[];
}
/** Owner commands acquire their registration identity in the supervisor, not in Host consumers. */
export type TunnelWorkerInput = TunnelWorkerCommand extends infer Command ? Command extends TunnelWorkerCommand ? Omit<Command, 'registrationToken'> : never : never;
/** Runtime options for one owner-side worker. */
export interface TunnelWorkerOptions {
    readonly command: TunnelProcessCommand;
}
/**
 * Own one long-lived tunnel process and multiplex all shares over IPC.
 * The worker is started lazily; commands issued before `ready` are queued.
 *
 * The process is supervised, not just launched: a worker that exits is replaced on the shared
 * reconnect schedule and every share the owner registered is handed to the replacement, so a crash
 * cannot leave a share offline that the owner never stopped sharing.
 */
export declare class TunnelWorker {
    private readonly options;
    private readonly listeners;
    private readonly queue;
    private readonly requests;
    /**
     * Shares the current process is expected to carry: registered by the owner, then confirmed or
     * dropped by the worker's own events. A share that leaves this set is not necessarily forgotten —
     * `registrations` below is what the owner still holds.
     */
    private readonly shares;
    /** Current add identity and latest committed Relay intent, retained across process replacements. */
    private readonly registrations;
    private child;
    private out;
    private ready;
    private stopping;
    private aliveSince;
    private backoff;
    private restartTimer;
    constructor(options: TunnelWorkerOptions);
    /** Start the worker and its private log file. */
    start(): void;
    /**
     * Subscribe to worker lifecycle events.
     * @param listener Receives worker readiness, share, and failure events.
     * @returns Disposer that removes the listener.
     */
    onEvent(listener: (event: TunnelWorkerEvent) => void): () => void;
    /**
     * Queue one command for the worker, starting it when necessary.
     *
     * The command is held until a worker can take it. While no process is running — before the first
     * start, or between a crash and its scheduled replacement — the restart replays the registration
     * set instead, which already carries this command's effect.
     * @param command Owner command to deliver after the worker is ready.
     * @returns Nothing.
     */
    send(command: TunnelWorkerInput): void;
    /** Stop accepting shares, cancel a pending restart, send shutdown, then kill a hung worker. */
    stop(): Promise<void>;
    private onMessage;
    private onExit;
    /** Replace the exited process after the scheduled delay, re-registering the owner's shares. */
    private scheduleRestart;
    /**
     * Hand every registered share to the process about to start.
     *
     * The registration set is the owner's current intent, so it replaces any `add-share` still queued
     * from before the crash: that command's spec may already be superseded, and a share the owner
     * removed in the meantime is no longer in the set at all.
     */
    /** The registration the token guard above already matched for one non-ready event. */
    private registrationOf;
    private replayRegistrations;
    private cancelRequests;
    private emit;
}
//# sourceMappingURL=tunnel-worker.d.ts.map