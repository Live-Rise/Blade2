/** Assistant stream baselines and per-connection subscriptions. */
import { SessionSeq } from '@deepseek-ai/dsh-session';
import { SessionAssistantStreamAccumulator } from "../assistant-stream.js";
/** One accumulator per live Session, shared by every subscription bound to it. */
export class SessionAssistantStreamsListener {
    ctx;
    streams = new Map();
    /** @param ctx - service context owning the accumulator listeners. */
    constructor(ctx) {
        this.ctx = ctx;
        ctx.on('agent/assistant-stream', ({ agent, frame }) => {
            let stream = this.streams.get(agent.session.id);
            if (stream === undefined) {
                stream = new SessionAssistantStreamAccumulator();
                this.streams.set(agent.session.id, stream);
            }
            stream.accept(frame, agent.session.seq === 0 ? -1 : SessionSeq(agent.session.seq - 1));
        }, { global: true });
        ctx.on('agent/disposed', ({ agent }) => { this.streams.delete(agent.session.id); }, { global: true });
        ctx.effect(() => () => { this.streams.clear(); }, 'session-controller.assistant-streams');
    }
    /**
     * Bind one connection to a Session assistant stream.
     * @param sessionId - Session to subscribe.
     * @param signal - cancellation owned by the caller.
     * @param queue - caller-owned raw notification buffer.
     * @returns a listener the caller disposes.
     */
    bind(sessionId, signal, queue) {
        return new AssistantStreamListener(this.ctx, sessionId, signal, () => {
            return this.streams.get(sessionId)?.snapshot() ?? { revision: 0 };
        }, queue);
    }
}
/** Writes assistant notifications into the connection's shared queue. */
class AssistantStreamListener {
    _snapshot;
    disposes = [];
    ordinal = 0;
    cut = 0;
    constructor(ctx, sessionId, signal, _snapshot, output) {
        this._snapshot = _snapshot;
        signal.throwIfAborted();
        const input = output.createInput(item => this.transform(item));
        try {
            this.disposes.push(ctx.on('agent/assistant-stream', ({ agent, frame }) => {
                if (agent.session.id !== sessionId)
                    return;
                output.capture(() => {
                    const wire = frame.type === 'start'
                        ? { ...frame, startedAfterSeq: agent.session.seq === 0 ? -1 : SessionSeq(agent.session.seq - 1) }
                        : frame.type === 'chunk' ? { ...frame, chunk: frame.chunk } : frame;
                    input.append({ type: 'assistant-stream', session: agent.session, frame: wire, ordinal: ++this.ordinal });
                });
            }, { global: true }));
        }
        catch (error) {
            this[Symbol.dispose]();
            throw error;
        }
    }
    /** @returns the baseline and local arrival cut captured synchronously together. */
    snapshot() {
        return { baseline: this._snapshot(), ordinal: this.ordinal };
    }
    /** @param ordinal - local snapshot cut set before queue consumption. */
    setOrdinalCut(ordinal) {
        this.cut = ordinal;
    }
    *transform(item) {
        if (this.disposes.length === 0 || item.ordinal <= this.cut)
            return;
        yield { type: 'assistant-stream', frame: item.frame };
    }
    [Symbol.dispose]() {
        for (const dispose of this.disposes.splice(0).reverse())
            dispose();
    }
}
//# sourceMappingURL=assistant-stream.js.map