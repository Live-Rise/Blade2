/** Passive per-Session streams; subscriptions never acquire Session observations. */
import type { Context } from '@deepseek-ai/cordis';
import type { Session, SessionEvent, SessionId } from '@deepseek-ai/dsh-session';
import type { SessionProjectionChange, SessionEventEntry } from '../types.ts';
/** Raw history notifications buffered before an opening cut is known. */
export type EventItem = {
    readonly type: 'event';
    readonly session: Session;
    readonly event: SessionEvent;
} | {
    readonly type: 'created';
    readonly session: Session;
    readonly end: number;
};
/** A single subscription buffers before either consumption mode starts. */
export declare class EventListener implements Disposable {
    private readonly buffer;
    private readonly disposes;
    private firstReceivedSeq;
    private nextExpectedSeq;
    /**
     * @param ctx - context owning the event registrations.
     * @param sessionId - Session whose notifications are emitted.
     * @param signal - cancellation that wakes consumers.
     * @param queue - optional caller-owned buffer; only an internally created buffer is disposed here.
     */
    constructor(ctx: Context, sessionId: SessionId, signal: AbortSignal, queue?: Pick<ListenerBuffer<SessionEventEntry>, 'createInput' | 'capture'>);
    /** Consume notifications without reading history.
     * @returns event frames from the first received event, without loading a baseline. */
    events(): AsyncIterable<SessionEventEntry>;
    /** Set the inclusive opening cut for buffered history notifications.
     * @param cursor - snapshot cut set before consuming the queue. */
    setCursor(cursor: number): void;
    private transform;
    private iterate;
    private recordFirst;
    [Symbol.dispose](): void;
}
/**
 * Select the uncovered events represented by one notification.
 * @param item - one buffered notification.
 * @param next - next required seq, or undefined before the first event.
 * @returns uncovered events and the next required seq without acquiring an observation.
 */
export declare function eventItem(item: EventItem, next?: number): {
    events: readonly SessionEvent[];
    next: number | undefined;
};
/** Projection changes of one Session, one change per committed event that changed a projection. */
export declare class ProjectionListener implements Disposable {
    private readonly buffer;
    /**
     * @param ctx - context owning projection notifications.
     * @param sessionId - Session whose notifications are emitted.
     * @param signal - cancellation that removes registrations and wakes consumers.
     */
    constructor(ctx: Context, sessionId: SessionId, signal: AbortSignal);
    /** Consume per-event projection changes.
     * @returns validated change frames, one per committed event; projection seqs need not be contiguous. */
    changes(): AsyncIterable<SessionProjectionChange>;
    [Symbol.dispose](): void;
}
/** Owns buffering, callback failure and registration cleanup for one consumer. */
export declare class ListenerBuffer<T> implements Disposable {
    private readonly values;
    private readonly transformIds;
    private readonly transforms;
    private readonly cleanup;
    private wake;
    private closed;
    private failure;
    private consumed;
    constructor(ctx: Context, signal: AbortSignal);
    /**
     * Register cleanup, or run it immediately when already closed.
     * @param dispose - synchronous cleanup.
     */
    own(dispose: () => void): void;
    /**
     * Contain producer failure and wake the consumer with that failure.
     * @param action - synchronous producer operation.
     */
    capture(action: () => void): void;
    /**
     * Register a dequeue transform and obtain its write-only input.
     * @param transform - conversion bound once and evaluated when each input is dequeued.
     * @returns a write-only input sharing this buffer's storage and lifetime.
     * @throws when this buffer is already closed.
     */
    createInput<Input>(transform: (item: Input) => Iterable<T>): {
        append(item: Input): void;
        prepend(item: Input): void;
    };
    /**
     * Consume queued values once, releasing the buffer when iteration ends.
     * @returns transformed output values.
     */
    items(): AsyncIterable<T>;
    private throwIfFailed;
    /** @returns whether disposal ended this buffer while a transform was yielding. */
    private isClosed;
    private notify;
    [Symbol.dispose](): void;
}
//# sourceMappingURL=listener.d.ts.map