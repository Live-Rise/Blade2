import { type RemoteUplinkWindow } from '@deepseek-ai/dsh-typert-protocol';
import type { SubprocessTerminalActivity, SubprocessTerminalForeground, SubprocessTerminalHandle, SubprocessTerminalSignal } from '@deepseek-ai/dsh-subprocess';
import { type TerminalRetentionPolicy } from './retention.ts';
import type { TerminalEnvironment, TerminalInput, TerminalFrame, TerminalRetentionFrame, TerminalScreenMark, TerminalScreenPage, TerminalScreenRequest, WebTerminalInfo } from './types.ts';
/** Process lifetime is independent of follower and component lifetimes. */
export declare class BrowserTerminal {
    private readonly handle;
    info: WebTerminalInfo;
    private readonly maxBufferedBytes;
    private readonly release;
    private readonly screen;
    private readonly serializer;
    private readonly followers;
    private sequence;
    private operations;
    private readonly drained;
    /** Provider completion precedes output drainage and the published exit state. */
    private exited;
    private closing;
    private screenDisposed;
    private retention;
    private controller;
    private changes;
    private readonly waiters;
    private stateListener;
    /**
     * @param handle - allocated terminal process range.
     * @param info - initial metadata.
     * @param scrollback - maximum retained scrollback rows.
     * @param maxBufferedBytes - per-follower queue cap.
     * @param release - removes files the launch created once the process has ended; it must not throw.
     */
    constructor(handle: SubprocessTerminalHandle, info: WebTerminalInfo, scrollback: number, maxBufferedBytes: number, release?: () => Promise<void>);
    /**
     * Start monitoring after this allocation is committed to its Session owner.
     * @param policy - validated Host timing policy.
     * @param closing - closes the id before any asynchronous termination.
     * @param closed - removes the exact successfully terminated owner record.
     * @param failed - diagnostic sink for background cleanup failure.
     */
    monitor(policy: TerminalRetentionPolicy, closing: () => void, closed: () => void, failed: (error: unknown) => void): void;
    /**
     * Retain this committed process independently of output attachment.
     * @param signal - physical window stream lifetime.
     * @returns its hold acknowledgement and lifetime.
     */
    retain(signal: AbortSignal): AsyncIterable<TerminalRetentionFrame>;
    /**
     * Attach with exclusive input control; older attachments keep receiving output.
     * @param input - Gateway-decoded commands; returning its iterator must interrupt pending reads.
     * @param limits - input byte and PTY dimension limits validated at the Remote boundary.
     * @param signal - attachment cancellation; never terminates the process.
     * @param inputWindow - carrier frame capacity; absent for a carrier without an encoded-frame inbox limit.
     * @returns a screen baseline, output, per-attachment state and ordered command acknowledgements.
     */
    attach(input: AsyncIterable<TerminalInput>, limits: Pick<TerminalEnvironment, 'maxInputBytes' | 'maxCols' | 'maxRows'>, signal: AbortSignal, inputWindow?: RemoteUplinkWindow): AsyncIterable<TerminalFrame>;
    private consumeInput;
    private writable;
    private broadcastState;
    /**
     * Set the one listener that reports published state changes to the owner's roster; a later call replaces it.
     * @param listener - called after each state publication; it must not throw.
     */
    setStateListener(listener: () => void): void;
    /**
     * Observe shell lifecycle without interpreting output.
     * @returns idle, busy, or unknown for unsupported shells and incomplete observations.
     */
    inspectActivity(): Promise<SubprocessTerminalActivity['state']>;
    /** Top-level terminal process id reported by the provider. */
    get pid(): number;
    /** Advances after each applied output chunk and each published state change. */
    get revision(): number;
    /**
     * Wait until output or state changes after a known revision.
     * @param after - revision the caller has observed.
     * @param signal - wait cancellation.
     * @returns the current revision, which exceeds `after`.
     */
    changed(after: number, signal: AbortSignal): Promise<number>;
    private advance;
    /**
     * Write agent input in order with browser input and output.
     * @param data - text delivered without implicit newline conversion.
     * @returns after the provider accepts the input.
     * @throws AgentTerminalError when the user controls the terminal or it is not running.
     */
    agentWrite(data: string): Promise<void>;
    /**
     * Deliver a signal to the foreground process group on the agent's behalf.
     * @param signal - permitted terminal signal.
     * @returns the process group that received it.
     * @throws AgentTerminalError when the user controls the terminal or it is not running.
     */
    agentSignal(signal: SubprocessTerminalSignal): Promise<number>;
    /**
     * Inspect the foreground process group without changing input control.
     * @returns the group and its input-wait fact, or undefined when the provider cannot resolve one.
     */
    inspectForeground(): Promise<SubprocessTerminalForeground | undefined>;
    /**
     * Wait until output received before the call, and input queued before it, are applied.
     * @returns after the queued work settles, whether or not it failed.
     */
    drain(): Promise<void>;
    /**
     * Render a newest-relative page of the screen as applied so far; {@link drain} first to include received output.
     * @param request - page position and byte bound.
     * @returns rendered text; alternate-screen applications return their current screen.
     * @throws AgentTerminalError after the screen is released.
     */
    readScreen(request: TerminalScreenRequest): TerminalScreenPage;
    /**
     * Mark the start of the cursor's logical line on the screen as applied so far; {@link drain} first to place it after received output.
     * @returns a mark whose lines begin with that logical line; a change of active buffer renders the whole new buffer.
     * @throws AgentTerminalError after the screen is released.
     */
    markScreen(): TerminalScreenMark;
    private requireAgentControl;
    /** Whether closing released the screen; reads and marks then fail. */
    get screenReleased(): boolean;
    private requireScreen;
    /**
     * Publish a display name to every attached view.
     * @param title - validated user title.
     */
    rename(title: string): void;
    /**
     * Terminate the complete provider-owned process range before releasing its screen.
     * @returns after process cleanup and final output drainage; failures remain retryable.
     */
    close(): Promise<void>;
    /**
     * Stop unattended cleanup scheduling and await final process cleanup.
     * @returns after terminal and monitor quiescence.
     */
    dispose(): Promise<void>;
    private closeProcess;
    private requireController;
    private broadcast;
    private enqueue;
    private consume;
    private output;
}
//# sourceMappingURL=terminal.d.ts.map