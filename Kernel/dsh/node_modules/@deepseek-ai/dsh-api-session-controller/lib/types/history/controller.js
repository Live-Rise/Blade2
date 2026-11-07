/** Independent Session snapshots and passive event subscriptions. */
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
import { SessionLogOffset, SessionSeq, isAppendSurfaceEvent } from '@deepseek-ai/dsh-session';
import { RemoteError, remoteErrorOf } from '@deepseek-ai/dsh-typert-protocol';
import { DEFAULT_MAX_MESSAGES, paginate, pageRecords, entryFor, addressId, validateHistoryWindow, validateAddress, rejectNotFound, } from "../history.js";
import { EventListener, ProjectionListener } from "./listener.js";
import { SessionAssistantStreamsListener } from "./assistant-stream.js";
/** Session snapshots do not share lifetime with passive subscriptions. */
export class HistoryController {
    ctx;
    /** Reconnect state for this controller's assistant subscriptions. */
    streamsListener;
    /** @param ctx - Host context carrying Session reads and projections. */
    constructor(ctx) {
        this.ctx = ctx;
        this.streamsListener = new SessionAssistantStreamsListener(this.ctx);
    }
    /**
     * Read a passive page of Session events at an exact cut.
     * @param request - Session identity and optional cut and page window.
     * @param signal - read cancellation.
     * @returns a page at the supplied cut, or the observed log end when omitted.
     */
    async events(request, signal) {
        const spec = resolveHistoryRequest(request);
        try {
            return await this.usingObservation({ sessionId: addressId(request.address), projection: request.address.kind === 'subagent' }, signal, (observation) => {
                validateObservationAddress(request.address, observation);
                return history(observation, spec);
            });
        }
        catch (error) {
            if (remoteErrorOf(error)?.code === 'session/not-found')
                rejectNotFound(request.address);
            throw error;
        }
    }
    /**
     * Subscribe to Session events without reading or activating the Session.
     * @param request - Session address to subscribe.
     * @param signal - stream cancellation.
     * @returns subscription readiness followed by event frames; readiness does not establish existence.
     */
    async *observeEvent(request, signal) {
        const env_1 = { stack: [], error: void 0, hasError: false };
        try {
            const listener = __addDisposableResource(env_1, new EventListener(this.ctx, addressId(request.address), signal), false);
            yield { type: 'ready' };
            yield* listener.events();
        }
        catch (e_1) {
            env_1.error = e_1;
            env_1.hasError = true;
        }
        finally {
            __disposeResources(env_1);
        }
    }
    /**
     * Read exact projections or migration cache hints without activating the Session.
     * @param request - Session identity whose exact projection baseline is required.
     * @param signal - read cancellation.
     * @returns exact sequenced values, migration cache hints, or null when absent.
     */
    async projections(request, signal) {
        const { sessionId } = request;
        if (sessionId.length === 0) {
            throw new RemoteError('gateway/bad-request', 'sessionId must not be empty', {});
        }
        try {
            signal.throwIfAborted();
            const live = this.ctx.sessions.get(sessionId);
            if (live !== undefined) {
                const snapshot = this.ctx.sessionProjections.snapshot(live);
                return { kind: 'sequenced', asOfSeq: snapshot.asOfSeq, values: snapshot.values };
            }
            else {
                const stored = await this.ctx.get('sessionPersistence')?.stat(sessionId, { signal });
                signal.throwIfAborted();
                if (stored?.formatStatus === 'migration-required') {
                    const cached = this.ctx.get('sessionProjectionCache')?.cachedSnapshot(stored.header);
                    return { kind: 'migration-required', values: { ...cached?.values } };
                }
                return await this.usingObservation({ sessionId: request.sessionId, projection: true }, signal, observation => ({
                    kind: 'sequenced',
                    ...projections(observation),
                }));
            }
        }
        catch (error) {
            const code = typeof error === 'object' && error !== null && 'code' in error ? error.code : undefined;
            if (code === 'SESSION_QUERY_SESSION_NOT_FOUND' || code === 'session/not-found') {
                return null;
            }
            if (signal.aborted || code === 'SESSION_QUERY_ABORTED') {
                throw new RemoteError('gateway/cancelled', 'Session projection read was cancelled', {}, { cause: error });
            }
            if (remoteErrorOf(error) !== undefined)
                throw error;
            throw new RemoteError('gateway/internal', 'Session projection read failed', {}, { cause: error });
        }
    }
    /**
     * Subscribe to projection changes without reading a baseline.
     * @param request - Session identity to subscribe without reading a baseline.
     * @param signal - stream cancellation.
     * @returns readiness followed by one change frame per committed event, listing every key it changed.
     */
    async *observeProjections(request, signal) {
        const env_2 = { stack: [], error: void 0, hasError: false };
        try {
            const listener = __addDisposableResource(env_2, new ProjectionListener(this.ctx, request.sessionId, signal), false);
            yield { type: 'ready' };
            yield* listener.changes();
        }
        catch (e_2) {
            env_2.error = e_2;
            env_2.hasError = true;
        }
        finally {
            __disposeResources(env_2);
        }
    }
    /**
     * Read bounded committed assistant messages through a stopping cursor.
     * @param sessionId - Session whose committed assistant messages are requested.
     * @param afterSeq - exclusive lower bound.
     * @param throughSeq - inclusive stopping-state cursor.
     * @param maxMessages - positive assistant record limit.
     * @param signal - caller cancellation, independent of a completed wait deadline.
     * @returns newest-first assistant messages and whether further matching records were omitted.
     */
    async assistantOutput(sessionId, afterSeq, throughSeq, maxMessages, signal) {
        return this.usingObservation({ sessionId, projection: false }, signal, (observation) => {
            if (throughSeq > observation.cursor) {
                throw new RemoteError('gateway/internal', 'Session history no longer covers the wait cursor', {});
            }
            const records = [];
            const events = observation.events;
            for (let index = throughSeq; index > afterSeq; index--) {
                const event = events[index];
                if (event.type !== 'assistant/message' || !isAppendSurfaceEvent(event))
                    continue;
                if (records.length === maxMessages)
                    return { records, truncated: true };
                records.push(entryFor(event));
            }
            return { records, truncated: false };
        });
    }
    async usingObservation({ sessionId, projection }, signal, callback) {
        const env_3 = { stack: [], error: void 0, hasError: false };
        try {
            let observation;
            try {
                observation = await this.ctx.sessionQuery.observeSession(sessionId, { signal, projectionMode: projection ? 'all' : 'none' });
            }
            catch (error) {
                const code = error?.code;
                if (code === 'SESSION_QUERY_SESSION_NOT_FOUND') {
                    throw new RemoteError('session/not-found', `session "${sessionId}" not found`, { sessionId });
                }
                if (code === 'SESSION_QUERY_ABORTED') {
                    // Keep the query's own message: cancellation detail was visible before this mapping.
                    throw new RemoteError('gateway/cancelled', error instanceof Error ? error.message : 'Session read was cancelled', {}, { cause: error });
                }
                throw error;
            }
            try {
                signal.throwIfAborted();
            }
            catch (error) {
                observation[Symbol.dispose]();
                throw error;
            }
            if (callback === undefined)
                return observation;
            const ownedObservation = __addDisposableResource(env_3, observation, false);
            return await callback(ownedObservation);
        }
        catch (e_3) {
            env_3.error = e_3;
            env_3.hasError = true;
        }
        finally {
            __disposeResources(env_3);
        }
    }
}
/**
 * Validate an Address against the exact read used by a Controller.
 * @param address - caller-supplied Session address.
 * @param observation - borrowed read; subagent addresses require projections.
 */
export function validateObservationAddress(address, observation) {
    if (observation.header.cwd === undefined)
        rejectNotFound(address);
    validateAddress(address, observation.header, observation.inheritedEventCount, observation.projections);
}
/**
 * Convert a borrowed observation to one page without acquiring or retaining it.
 * @param observation - exact read cut owned by the caller.
 * @param request - validated page window and optional inclusive cut.
 * @returns a chronological page at the requested cut.
 */
export function history(observation, request) {
    const cursor = request.throughSeq === undefined ? observation.cursor
        : request.throughSeq === -1 ? -1 : SessionSeq(request.throughSeq);
    if (cursor > observation.cursor) {
        throw new RemoteError('gateway/bad-request', `session history cut ${String(cursor)} exceeds ${String(observation.cursor)}`, {});
    }
    const page = paginate(observation.events, request.beforeSeq === undefined ? undefined : SessionLogOffset(request.beforeSeq), request.maxMessages, cursor, request.turnWindow);
    return { cursor, records: pageRecords(page.events), hasMore: page.hasMore };
}
/**
 * Convert the projection baseline without acquiring or retaining its observation.
 * @param observation - projected read cut owned by the caller.
 * @returns the exact baseline; missing projection support fails explicitly.
 */
export function projections(observation) {
    const snapshot = observation.projections;
    if (snapshot === undefined)
        throw new RemoteError('session/projections-unavailable', 'Session projections are unavailable', {});
    return { asOfSeq: snapshot.asOfSeq, values: snapshot.values };
}
/**
 * Validate and resolve the history page window.
 * @param request - external history window.
 * @returns the validated request with an explicit message budget.
 */
export function resolveHistoryRequest(request) {
    if (request.throughSeq !== undefined
        && (!Number.isSafeInteger(request.throughSeq) || request.throughSeq < -1 || Object.is(request.throughSeq, -0))) {
        throw new RemoteError('gateway/bad-request', 'throughSeq must be an integer greater than or equal to -1', {});
    }
    if (request.beforeSeq !== undefined
        && (!Number.isSafeInteger(request.beforeSeq) || request.beforeSeq < 0 || Object.is(request.beforeSeq, -0))) {
        throw new RemoteError('gateway/bad-request', 'beforeSeq must be a non-negative safe integer', {});
    }
    validateHistoryWindow(request);
    return { ...request, maxMessages: request.maxMessages ?? DEFAULT_MAX_MESSAGES };
}
//# sourceMappingURL=controller.js.map