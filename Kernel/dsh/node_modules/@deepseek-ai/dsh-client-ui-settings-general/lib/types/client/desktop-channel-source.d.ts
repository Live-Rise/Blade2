import type { DesktopChannelBridge, DesktopChannelState, DesktopUpdateChannel, DesktopChannelChangeResult } from '../types.ts';
type Notice = {
    sequence: number;
    outcome: 'configuration' | 'applied' | Extract<DesktopChannelChangeResult, {
        kind: 'failed';
    }>['reason'];
};
/** Actual shell state, optimistic selection and transient operation feedback. */
export interface DesktopChannelSnapshot {
    state?: DesktopChannelState;
    failed: boolean;
    pending?: DesktopUpdateChannel;
    notice?: Notice;
}
/** Client observation; no Host service or configuration file is read here. */
export declare class DesktopChannelSource {
    private readonly bridge;
    /** Stable observation shared by mounted General views and the shell toast. */
    readonly store: import("@deepseek-ai/dsh-client-store").SnapshotStore<DesktopChannelSnapshot>;
    private live;
    private revision;
    private sequence;
    private readonly unsubscribe;
    /** @param bridge - Isolated shell API, present only in Desktop. */
    constructor(bridge: DesktopChannelBridge);
    private receive;
    /**
     * Show a pending choice until the shell reports its actual selection.
     * @param channel - Selected feed.
     * @returns After the shell reports the result.
     */
    select(channel: DesktopUpdateChannel): Promise<void>;
    /**
     * Retire only the toast whose animation completed.
     * @param sequence - Notice identity captured by the rendered toast.
     */
    dismissNotice(sequence: number): void;
    private isLive;
    /** Detach the shell subscription and ignore late IPC responses. */
    dispose(): void;
}
export {};
//# sourceMappingURL=desktop-channel-source.d.ts.map