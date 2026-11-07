/** Web opening aggregation over one observation and a shared transforming buffer. */
var __addDisposableResource = (this && this.__addDisposableResource) || function (env, value, async) {
    if (value !== null && value !== void 0) {
        if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
        var dispose, inner;
        if (async) {
            if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
            dispose = value[Symbol.asyncDispose];
        }
        if (dispose === void 0) {
            if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
            dispose = value[Symbol.dispose];
            if (async) inner = dispose;
        }
        if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
        if (inner) dispose = function() { try { inner.call(this); } catch (e) { return Promise.reject(e); } };
        env.stack.push({ value: value, dispose: dispose, async: async });
    }
    else if (async) {
        env.stack.push({ async: true });
    }
    return value;
};
var __disposeResources = (this && this.__disposeResources) || (function (SuppressedError) {
    return function (env) {
        function fail(e) {
            env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
            env.hasError = true;
        }
        var r, s = 0;
        function next() {
            while (r = env.stack.pop()) {
                try {
                    if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
                    if (r.dispose) {
                        var result = r.dispose.call(r.value);
                        if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) { fail(e); return next(); });
                    }
                    else s |= 1;
                }
                catch (e) {
                    fail(e);
                }
            }
            if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
            if (env.hasError) throw env.error;
        }
        return next();
    };
})(typeof SuppressedError === "function" ? SuppressedError : function (error, suppressed, message) {
    var e = new Error(message);
    return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
import { remoteErrorOf } from '@deepseek-ai/dsh-typert-protocol';
import { addressId, rejectNotFound } from "../history.js";
import { EventListener, ListenerBuffer } from "../history/listener.js";
import { history, projections, resolveHistoryRequest, validateObservationAddress } from "../history/controller.js";
/** Combines opening data and consumes transformed listener output from one queue. */
export class WebController {
    ctx;
    historyController;
    promote;
    /**
     * @param ctx - Host context carrying Session queries and identity projections.
     * @param historyController - passive reading and listener construction.
     * @param promote - background activation requested before an opening snapshot is delivered.
     */
    constructor(ctx, historyController, promote) {
        this.ctx = ctx;
        this.historyController = historyController;
        this.promote = promote;
    }
    /**
     * Open a history snapshot and follow uncovered history and assistant notifications.
     * @param request - Session identity, page budget, assistant output and optional activation (default false).
     * @param signal - cancellation for opening and streaming.
     * @returns an opening snapshot followed by uncovered queued increments.
     */
    async *follow(request, signal) {
        const env_1 = { stack: [], error: void 0, hasError: false };
        try {
            const sessionId = addressId(request.address);
            const spec = resolveHistoryRequest(request);
            const queue = __addDisposableResource(env_1, new ListenerBuffer(this.ctx, signal), false);
            const listener = __addDisposableResource(env_1, new EventListener(this.ctx, sessionId, signal, queue), false);
            const streams = this.historyController.streamsListener;
            const assistantListener = __addDisposableResource(env_1, request.assistantStream === true
                ? streams.bind(sessionId, signal, queue)
                : undefined, false);
            const observation = __addDisposableResource(env_1, await this.historyController.usingObservation({ sessionId, projection: true }, signal)
                .catch((error) => {
                if (remoteErrorOf(error)?.code === 'session/not-found')
                    rejectNotFound(request.address);
                throw error;
            }), false);
            validateObservationAddress(request.address, observation);
            let opening = {
                type: 'snapshot',
                header: { ...observation.header },
                projections: projections(observation),
                ...history(observation, spec),
            };
            listener.setCursor(opening.cursor);
            if (assistantListener !== undefined) {
                const { baseline: assistantStream, ordinal } = assistantListener.snapshot();
                opening = { ...opening, assistantStream };
                assistantListener.setOrdinalCut(ordinal);
            }
            // Increments come from the queue, so the opening ends this lease in every branch;
            // activation continues on its own retained lease. Promotion starts before the opening is
            // delivered, so a consumer that stops after the snapshot still gets the activation it asked for.
            if (request.activate === true && !signal.aborted
                && observation.header.origin !== 'subagent' && observation.source === 'prepared') {
                const promotion = observation.retain();
                observation[Symbol.dispose]();
                try {
                    this.promote(promotion);
                }
                catch (error) {
                    promotion[Symbol.dispose]();
                    throw error;
                }
            }
            else {
                observation[Symbol.dispose]();
            }
            yield opening;
            yield* queue.items();
        }
        catch (e_1) {
            env_1.error = e_1;
            env_1.hasError = true;
        }
        finally {
            __disposeResources(env_1);
        }
    }
}
//# sourceMappingURL=controller.js.map