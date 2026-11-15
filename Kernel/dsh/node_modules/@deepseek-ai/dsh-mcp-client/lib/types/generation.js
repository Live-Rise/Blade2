/** One transport lifetime, including pending work and confirmed closure. */
import { withDeadline } from "./deadline.js";
import { McpLocalError } from "./diagnostic.js";
import { createTransport } from "./transport.js";
/** Tracks actual tasks separately from their interruptible callers. */
export class McpGeneration {
    client;
    closeTimeoutMs;
    diagnostic;
    /** Cancellation shared by the transport and all work owned by this generation. */
    lifetime = new AbortController();
    pending = new Set();
    closed = Promise.withResolvers();
    transport;
    started = false;
    closeObserved = false;
    closing;
    constructor(client, closeTimeoutMs, diagnostic) {
        this.client = client;
        this.closeTimeoutMs = closeTimeoutMs;
        this.diagnostic = diagnostic;
    }
    /**
     * Connect within the caller's remaining startup or probe budget.
     * @param config - resolved transport configuration.
     * @param timeoutMs - remaining operation budget; undefined leaves the SDK default.
     * @returns completion of negotiation, or cancellation/deadline rejection.
     */
    connect(config, timeoutMs) {
        this.lifetime.signal.throwIfAborted();
        const transport = this.transport = createTransport(config);
        const close = transport.close.bind(transport);
        let closing;
        transport.close = () => closing ??= close();
        const start = transport.start.bind(transport);
        transport.start = () => {
            this.lifetime.signal.throwIfAborted();
            this.started = true;
            return start();
        };
        transport.onclose = () => {
            this.closeObserved = true;
            this.closed.resolve();
        };
        return this.run(async (signal) => {
            await this.client.connect(transport, { signal,
                ...timeoutMs === undefined ? {} : { timeout: Math.max(1, timeoutMs) },
            });
            this.assertOpen();
        }, timeoutMs);
    }
    /** Reject when this generation was cancelled or its transport reported closure. */
    assertOpen() {
        this.lifetime.signal.throwIfAborted();
        if (this.closeObserved)
            throw new McpLocalError('MCP transport closed');
    }
    /**
     * Track work through settlement, even when its caller times out or is cancelled.
     * @param operation - work receiving this generation's cancellation signal.
     * @param timeoutMs - optional caller deadline in milliseconds; does not cancel the work.
     * @returns the operation's result, or a cancellation/deadline rejection.
     */
    run(operation, timeoutMs) {
        const signal = this.lifetime.signal;
        const task = Promise.resolve().then(() => {
            signal.throwIfAborted();
            return operation(signal);
        });
        this.pending.add(task);
        void task.then(() => this.pending.delete(task), () => this.pending.delete(task));
        const interrupted = new Promise((resolve, reject) => {
            const abort = () => { reject(signal.reason); };
            signal.addEventListener('abort', abort, { once: true });
            if (signal.aborted)
                abort();
            void task.then(resolve, reject).finally(() => { signal.removeEventListener('abort', abort); });
        });
        return timeoutMs === undefined ? interrupted : withDeadline(interrupted, timeoutMs, 'MCP operation timed out');
    }
    /**
     * Cancel callers, close the transport, and await owned work within the cleanup budget.
     * @returns the shared cleanup result, distinguishing transport closure from cleanup failure.
     */
    close() {
        return this.closing ??= this.finish();
    }
    async finish() {
        this.lifetime.abort(new McpLocalError('MCP connection closed'));
        let cleanupError;
        const work = async () => {
            try {
                await (this.client.transport === undefined ? this.transport?.close() : this.client.close());
            }
            catch (error) {
                cleanupError = this.diagnostic(error);
            }
            await Promise.allSettled([...this.pending]);
            if (this.started && !this.closeObserved)
                await this.closed.promise;
        };
        try {
            await withDeadline(work(), this.closeTimeoutMs, 'MCP cleanup did not settle');
            return { closeConfirmed: true, ...cleanupError === undefined ? {} : { cleanupError } };
        }
        catch (error) {
            return { closeConfirmed: this.transport === undefined || this.closeObserved, cleanupError: cleanupError ?? this.diagnostic(error) };
        }
    }
}
//# sourceMappingURL=generation.js.map