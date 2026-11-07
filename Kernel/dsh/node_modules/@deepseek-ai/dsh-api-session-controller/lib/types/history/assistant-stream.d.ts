/** Assistant stream baselines and per-connection subscriptions. */
import type { Context } from '@deepseek-ai/cordis';
import type { SessionId } from '@deepseek-ai/dsh-session';
import type { SessionAssistantStreamBaseline, SessionAssistantStreamFrame } from '../types.ts';
import { ListenerBuffer } from './listener.ts';
/** One accumulator per live Session, shared by every subscription bound to it. */
export declare class SessionAssistantStreamsListener {
    private readonly ctx;
    private readonly streams;
    /** @param ctx - service context owning the accumulator listeners. */
    constructor(ctx: Context);
    /**
     * Bind one connection to a Session assistant stream.
     * @param sessionId - Session to subscribe.
     * @param signal - cancellation owned by the caller.
     * @param queue - caller-owned raw notification buffer.
     * @returns a listener the caller disposes.
     */
    bind(sessionId: SessionId, signal: AbortSignal, queue: Pick<ListenerBuffer<AssistantEntry>, 'createInput' | 'capture'>): AssistantStreamListener;
}
type AssistantEntry = {
    type: 'assistant-stream';
    frame: SessionAssistantStreamFrame;
};
/** Writes assistant notifications into the connection's shared queue. */
declare class AssistantStreamListener implements Disposable {
    private _snapshot;
    private readonly disposes;
    private ordinal;
    private cut;
    constructor(ctx: Context, sessionId: SessionId, signal: AbortSignal, _snapshot: () => SessionAssistantStreamBaseline, output: Pick<ListenerBuffer<AssistantEntry>, 'createInput' | 'capture'>);
    /** @returns the baseline and local arrival cut captured synchronously together. */
    snapshot(): {
        baseline: SessionAssistantStreamBaseline;
        ordinal: number;
    };
    /** @param ordinal - local snapshot cut set before queue consumption. */
    setOrdinalCut(ordinal: number): void;
    private transform;
    [Symbol.dispose](): void;
}
export {};
//# sourceMappingURL=assistant-stream.d.ts.map