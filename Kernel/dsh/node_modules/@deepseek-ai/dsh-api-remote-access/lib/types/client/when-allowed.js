/**
 * The effect controller behind `whenAccess`: install a callback's effects
 * while one access answer is true and dispose them while it is false.
 * @module @deepseek-ai/dsh-api-remote-access/src/client/when-allowed
 */
/**
 * Install `callback`'s effects while `answer` is true. The callback runs
 * synchronously when the answer is already true; a later true runs it again
 * after a false disposed the previous installation in reverse order. The
 * controller belongs to `ctx`'s fiber.
 * @param ctx - the caller's Context; it owns the controller and every installation.
 * @param answer - the access answer to follow.
 * @param callback - creates one disposer or an iterable of disposers.
 * @param label - effect label for diagnostics.
 * @returns idempotent disposer for the controller and any installed effects.
 * @throws callback setup failures synchronously when the answer is already true.
 */
export function whenAllowed(ctx, answer, callback, label) {
    const disposeController = ctx.effect(() => {
        let installed;
        let stopped = false;
        const uninstall = () => {
            const dispose = installed;
            installed = undefined;
            dispose?.();
        };
        const reconcile = () => {
            if (stopped)
                return;
            const allowed = answer.getSnapshot();
            if (allowed === (installed !== undefined))
                return;
            if (!allowed) {
                uninstall();
                return;
            }
            // Each installation is a nested Cordis effect, so an iterable callback
            // installs transactionally and disposes in reverse order.
            const disposeEffect = ctx.effect(callback, `${label}: allowed`);
            installed = () => { void disposeEffect(); };
        };
        const unsubscribe = answer.subscribe(() => { changed(); });
        const stop = () => {
            if (stopped)
                return;
            // A failed installation retires the controller: a later grant never retries it.
            stopped = true;
            unsubscribe();
            uninstall();
        };
        const changed = () => {
            try {
                reconcile();
            }
            catch (error) {
                stop();
                if (error?.code === 'INACTIVE_EFFECT')
                    return;
                const failure = error instanceof Error ? error : new Error(String(error));
                queueMicrotask(() => { throw failure; });
            }
        };
        try {
            reconcile();
        }
        catch (error) {
            stop();
            throw error;
        }
        return stop;
    }, label);
    return () => { void disposeController(); };
}
//# sourceMappingURL=when-allowed.js.map