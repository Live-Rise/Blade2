/** Coalesced Session state observations with Host-local revisions. */
/** Coalesces Session/Agent invalidations with local revisions; pending events are sampled on reads. */
export class StateListener {
    readState;
    disposes = [];
    cancellation = new AbortController();
    signal;
    revision = 0;
    resolver = createDeferredSignal();
    closed = false;
    /**
     * @param ctx - lifecycle notification owner.
     * @param target - Session to observe.
     * @param signal - caller cancellation.
     * @param readState - passive complete-state reader.
     */
    constructor(ctx, target, signal, readState) {
        this.readState = readState;
        signal.throwIfAborted();
        this.signal = AbortSignal.any([signal, this.cancellation.signal]);
        const notify = (id) => {
            if (id !== target || this.closed)
                return;
            this.revision++;
            const resolver = this.resolver;
            this.resolver = createDeferredSignal();
            resolver.resolve();
        };
        const close = () => { this[Symbol.dispose](); };
        signal.addEventListener('abort', close, { once: true });
        this.disposes.push(() => { signal.removeEventListener('abort', close); });
        try {
            this.disposes.push(ctx.on('session/event', (session) => { notify(session.id); }, { global: true }));
            this.disposes.push(ctx.on('session/created', (session) => { notify(session.id); }, { global: true }));
            this.disposes.push(ctx.on('session/disposed', (session) => { notify(session.id); }, { global: true }));
            this.disposes.push(ctx.on('agent/status', ({ agent }) => { notify(agent.id); }, { global: true }));
            this.disposes.push(ctx.on('agent/created', ({ agent }) => { notify(agent.id); }, { global: true }));
            this.disposes.push(ctx.on('agent/disposed', ({ agent }) => { notify(agent.id); }, { global: true }));
            const dispose = ctx.effect(() => close, 'session-controller.state-listener');
            this.disposes.push(() => { void dispose(); });
        }
        catch (error) {
            this[Symbol.dispose]();
            throw error;
        }
    }
    /**
     * Check whether a completed read still represents the latest invalidation.
     * @param revision - revision captured with a completed read.
     * @returns whether no intervening invalidation occurred.
     */
    isCurrent(revision) { return !this.closed && revision === this.revision; }
    /**
     * Read coalesced state replacements with optional periodic refresh.
     * @param refreshMs - optional idle recheck interval; omitted for event-only observation.
     * @returns complete state replacements with internal consistency revisions.
     */
    async *observeStates(refreshMs) {
        let lastRevision = -1;
        try {
            while (!this.closed) {
                if (lastRevision === this.revision) {
                    if (refreshMs === undefined) {
                        await this.resolver.promise;
                    }
                    else {
                        let timer;
                        try {
                            await Promise.race([
                                this.resolver.promise,
                                new Promise((resolve) => { timer = setTimeout(resolve, refreshMs); }),
                            ]);
                        }
                        finally {
                            clearTimeout(timer);
                        }
                        if (this.isCurrent(lastRevision)) {
                            this.revision++;
                            const resolver = this.resolver;
                            this.resolver = createDeferredSignal();
                            resolver.resolve();
                        }
                    }
                }
                if (!this.isCurrent(this.revision))
                    return;
                lastRevision = this.revision;
                const state = await this.readState(this.signal);
                if (this.isCurrent(lastRevision))
                    yield { state, revision: lastRevision };
            }
        }
        finally {
            this[Symbol.dispose]();
        }
    }
    [Symbol.dispose]() {
        if (this.closed)
            return;
        this.closed = true;
        this.cancellation.abort();
        this.resolver.resolve();
        for (const dispose of this.disposes.splice(0).reverse())
            dispose();
    }
}
function createDeferredSignal() {
    let resolve;
    const promise = new Promise((settle) => { resolve = settle; });
    return { promise, resolve };
}
//# sourceMappingURL=listener.js.map