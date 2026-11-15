/** One reference-counted terminal roster stream per watched Session. */
import { RemoteStreamCarrierError } from '@deepseek-ai/dsh-api-gateway/client';
/**
 * Shares one Host roster stream among a Session's watchers and stops it with the last watcher. A stream that fails
 * while watched reopens for its remaining watchers on the next watch of that Session or on {@link TerminalRosters.resume}.
 */
export class TerminalRosters {
    gateway;
    remote;
    received;
    failed;
    entries = new Map();
    stopping = new Set();
    /**
     * @param gateway - Client Remote carrier captured while the plugin context is current.
     * @param remote - generated terminal namespace.
     * @param received - shared state update applied before listeners run.
     * @param failed - diagnostic sink for listener and stream failures.
     */
    constructor(gateway, remote, received, failed) {
        this.gateway = gateway;
        this.remote = remote;
        this.received = received;
        this.failed = failed;
    }
    /**
     * Watch one Session's terminals.
     * @param sessionId - watched Session.
     * @param listener - called with every roster while watched.
     * @returns the release; the stream stops when its last watcher releases.
     */
    watch(sessionId, listener) {
        let entry = this.entries.get(sessionId);
        if (entry === undefined || entry.stopped) {
            entry = { listeners: new Set(), latest: undefined, stopped: false, dispose: undefined };
            this.entries.set(sessionId, entry);
        }
        if (entry.dispose === undefined)
            this.open(sessionId, entry);
        entry.listeners.add(listener);
        if (entry.latest !== undefined)
            this.notify(listener, entry.latest);
        const owned = entry;
        let released = false;
        return () => {
            if (released)
                return;
            released = true;
            owned.listeners.delete(listener);
            if (owned.listeners.size > 0)
                return;
            if (this.entries.get(sessionId) === owned)
                this.entries.delete(sessionId);
            this.stop(owned);
        };
    }
    /**
     * Reopen the streams that failed while watched, for example after the connection is replaced.
     */
    resume() {
        for (const [sessionId, entry] of this.entries) {
            if (entry.dispose === undefined)
                this.open(sessionId, entry);
        }
    }
    /**
     * Stop every stream.
     * @returns after every stream has closed.
     */
    async dispose() {
        for (const entry of this.entries.values())
            this.stop(entry);
        this.entries.clear();
        await Promise.all(this.stopping);
    }
    open(sessionId, entry) {
        const name = `terminal roster ${String(sessionId)}`;
        const stream = this.gateway.$stream({
            name,
            open: signal => this.remote.watch(sessionId, signal),
            // The roster has no natural end while watched: an end after the first frame is a carrier interruption.
            ended: accepted => accepted
                ? new RemoteStreamCarrierError(`${name} ended before release`)
                : new Error(`${name} ended before its first frame`),
        });
        const dispose = () => stream.dispose();
        entry.dispose = dispose;
        void (async () => {
            try {
                for await (const item of stream) {
                    entry.latest = item.value;
                    this.received(sessionId, item.value);
                    for (const listener of [...entry.listeners])
                        this.notify(listener, item.value);
                    item.accept();
                }
            }
            catch (error) {
                // Disposal ends the stream without an error, so every caught failure is a live stream failure.
                this.failed(error);
            }
            finally {
                if (entry.dispose === dispose) {
                    entry.dispose = undefined;
                    this.track(dispose());
                }
            }
        })();
    }
    notify(listener, terminals) {
        try {
            listener(terminals);
        }
        catch (error) {
            this.failed(error);
        }
    }
    stop(entry) {
        if (entry.stopped)
            return;
        entry.stopped = true;
        const dispose = entry.dispose;
        entry.dispose = undefined;
        if (dispose !== undefined)
            this.track(dispose());
    }
    track(disposal) {
        const stopping = disposal.finally(() => { this.stopping.delete(stopping); });
        this.stopping.add(stopping);
    }
}
//# sourceMappingURL=roster.js.map