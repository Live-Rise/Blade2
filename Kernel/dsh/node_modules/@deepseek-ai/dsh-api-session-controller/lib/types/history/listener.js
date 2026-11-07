/** Passive per-Session streams; subscriptions never acquire Session observations. */
import { Deque } from '@deepseek-ai/dsh-deque';
import { SessionLogOffset } from '@deepseek-ai/dsh-session';
import { RemoteError } from '@deepseek-ai/dsh-typert-protocol';
import { entryFor } from "../history.js";
/** A single subscription buffers before either consumption mode starts. */
export class EventListener {
    buffer;
    disposes = [];
    firstReceivedSeq;
    nextExpectedSeq;
    /**
     * @param ctx - context owning the event registrations.
     * @param sessionId - Session whose notifications are emitted.
     * @param signal - cancellation that wakes consumers.
     * @param queue - optional caller-owned buffer; only an internally created buffer is disposed here.
     */
    constructor(ctx, sessionId, signal, queue) {
        signal.throwIfAborted();
        const buffer = queue ?? new ListenerBuffer(ctx, signal);
        this.buffer = queue === undefined ? buffer : undefined;
        const input = buffer.createInput(item => this.transform(item));
        const target = sessionId;
        try {
            this.disposes.push(ctx.on('session/event', (session, event) => {
                if (session.id !== target)
                    return;
                buffer.capture(() => {
                    this.recordFirst(event.seq);
                    input.append({ type: 'event', session, event });
                });
            }, { global: true }));
            this.disposes.push(ctx.on('session/created', (session) => {
                if (session.id !== target)
                    return;
                buffer.capture(() => {
                    if (session.firstLiveSeq < session.seq)
                        this.recordFirst(session.firstLiveSeq);
                    input.prepend({ type: 'created', session, end: session.seq });
                });
            }, { global: true }));
            this.buffer?.own(() => { this[Symbol.dispose](); });
        }
        catch (error) {
            this[Symbol.dispose]();
            throw error;
        }
    }
    /** Consume notifications without reading history.
     * @returns event frames from the first received event, without loading a baseline. */
    events() {
        return this.iterate();
    }
    /** Set the inclusive opening cut for buffered history notifications.
     * @param cursor - snapshot cut set before consuming the queue. */
    setCursor(cursor) {
        this.nextExpectedSeq = cursor + 1;
    }
    *transform(item) {
        if (this.disposes.length === 0)
            return;
        this.nextExpectedSeq ??= this.firstReceivedSeq;
        const result = eventItem(item, this.nextExpectedSeq);
        this.nextExpectedSeq = result.next;
        for (const event of result.events) {
            if (this.disposes.length === 0)
                return;
            yield entryFor(event);
        }
    }
    async *iterate() {
        if (this.buffer === undefined)
            throw new Error('Consume the externally owned history buffer directly');
        try {
            yield* this.buffer.items();
        }
        finally {
            this[Symbol.dispose]();
        }
    }
    recordFirst(seq) {
        this.firstReceivedSeq = this.firstReceivedSeq === undefined ? seq : Math.min(this.firstReceivedSeq, seq);
    }
    [Symbol.dispose]() {
        for (const dispose of this.disposes.splice(0).reverse())
            dispose();
        this.buffer?.[Symbol.dispose]();
    }
}
/**
 * Select the uncovered events represented by one notification.
 * @param item - one buffered notification.
 * @param next - next required seq, or undefined before the first event.
 * @returns uncovered events and the next required seq without acquiring an observation.
 */
export function eventItem(item, next) {
    if (item.type === 'event') {
        if (next !== undefined && item.event.seq < next)
            return { events: [], next };
        if (next !== undefined && item.event.seq !== next) {
            throw new RemoteError('gateway/internal', `session event stream skipped seq ${String(next)}`, {});
        }
        return { events: [item.event], next: item.event.seq + 1 };
    }
    const start = next ?? item.session.firstLiveSeq;
    if (start >= item.end)
        return { events: [], next };
    // oxlint-disable-next-line typescript/no-deprecated -- Existing Session-created history adaptation.
    const events = item.session.snapshotEvents(SessionLogOffset(start), SessionLogOffset(item.end));
    for (const event of events) {
        if (next !== undefined && event.seq !== next) {
            throw new RemoteError('gateway/internal', `session event stream skipped seq ${String(next)}`, {});
        }
        next = event.seq + 1;
    }
    return { events, next };
}
/** Projection changes of one Session, one change per committed event that changed a projection. */
export class ProjectionListener {
    buffer;
    /**
     * @param ctx - context owning projection notifications.
     * @param sessionId - Session whose notifications are emitted.
     * @param signal - cancellation that removes registrations and wakes consumers.
     */
    constructor(ctx, sessionId, signal) {
        this.buffer = new ListenerBuffer(ctx, signal);
        const input = this.buffer.createInput(item => [item]);
        const target = sessionId;
        try {
            this.buffer.own(ctx.sessionProjections.onEventChanged((session, changes, seq) => {
                if (session.id !== target)
                    return;
                this.buffer.capture(() => {
                    // Every projection definition validates its value before change publication.
                    const change = { type: 'projection', seq, changes: changes };
                    input.append({ session, change });
                });
            }));
        }
        catch (error) {
            this.buffer[Symbol.dispose]();
            throw error;
        }
    }
    /** Consume per-event projection changes.
     * @returns validated change frames, one per committed event; projection seqs need not be contiguous. */
    async *changes() {
        try {
            for await (const { change } of this.buffer.items()) {
                yield change;
            }
        }
        finally {
            this[Symbol.dispose]();
        }
    }
    [Symbol.dispose]() {
        this.buffer[Symbol.dispose]();
    }
}
/** Owns buffering, callback failure and registration cleanup for one consumer. */
export class ListenerBuffer {
    values = new Deque();
    transformIds = new Deque();
    transforms = [];
    cleanup = [];
    wake;
    closed = false;
    failure;
    consumed = false;
    constructor(ctx, signal) {
        signal.throwIfAborted();
        const close = () => { this[Symbol.dispose](); };
        signal.addEventListener('abort', close, { once: true });
        this.own(() => { signal.removeEventListener('abort', close); });
        try {
            const dispose = ctx.effect(() => close, 'session-controller.listener-buffer');
            this.own(() => { void dispose(); });
        }
        catch (error) {
            this[Symbol.dispose]();
            throw error;
        }
    }
    /**
     * Register cleanup, or run it immediately when already closed.
     * @param dispose - synchronous cleanup.
     */
    own(dispose) {
        if (this.closed)
            dispose();
        else
            this.cleanup.push(dispose);
    }
    /**
     * Contain producer failure and wake the consumer with that failure.
     * @param action - synchronous producer operation.
     */
    capture(action) {
        if (this.closed)
            return;
        try {
            action();
        }
        catch (error) {
            this.failure = { error };
            this[Symbol.dispose]();
        }
    }
    /**
     * Register a dequeue transform and obtain its write-only input.
     * @param transform - conversion bound once and evaluated when each input is dequeued.
     * @returns a write-only input sharing this buffer's storage and lifetime.
     * @throws when this buffer is already closed.
     */
    createInput(transform) {
        if (this.closed)
            throw new Error('Cannot register a transform on a closed listener buffer');
        const transformId = this.transforms.length;
        // Only this typed input can enqueue values with its private transform id.
        this.transforms.push(value => transform(value));
        return {
            append: (item) => {
                if (this.closed)
                    return;
                this.values.pushBack(item);
                this.transformIds.pushBack(transformId);
                this.notify();
            },
            prepend: (item) => {
                if (this.closed)
                    return;
                this.values.pushFront(item);
                this.transformIds.pushFront(transformId);
                this.notify();
            },
        };
    }
    /**
     * Consume queued values once, releasing the buffer when iteration ends.
     * @returns transformed output values.
     */
    async *items() {
        if (this.consumed)
            throw new RemoteError('gateway/internal', 'Session listener already has a consumer', {});
        this.consumed = true;
        try {
            for (;;) {
                if (this.failure !== undefined)
                    throw this.failure.error;
                if (this.closed)
                    return;
                if (this.transformIds.size === 0) {
                    await new Promise((resolve) => { this.wake = resolve; });
                    continue;
                }
                const transformId = this.transformIds.popFront();
                const value = this.values.popFront();
                for (const item of this.transforms[transformId](value)) {
                    this.throwIfFailed();
                    if (this.isClosed())
                        return;
                    yield item;
                }
            }
        }
        finally {
            this[Symbol.dispose]();
        }
    }
    throwIfFailed() {
        if (this.failure !== undefined)
            throw this.failure.error;
    }
    /** @returns whether disposal ended this buffer while a transform was yielding. */
    isClosed() {
        return this.closed;
    }
    notify() {
        const wake = this.wake;
        this.wake = undefined;
        wake?.();
    }
    [Symbol.dispose]() {
        if (this.closed)
            return;
        this.closed = true;
        this.values.clear();
        this.transformIds.clear();
        this.transforms.length = 0;
        this.notify();
        for (const dispose of this.cleanup.splice(0).reverse())
            dispose();
    }
}
//# sourceMappingURL=listener.js.map