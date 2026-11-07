/** Passive Session state, coalesced observations and quiescence waits. */
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
import { deadline, MAX_TIMER_DELAY_MS, timeoutOf } from '@deepseek-ai/dsh-timeout';
import { RemoteError } from '@deepseek-ai/dsh-typert-protocol';
import { DEFAULT_MAX_MESSAGES, rejectNotFound } from "../history.js";
import { StateListener } from "./listener.js";
const DEFAULT_WAIT_TIMEOUT_MS = 30_000;
/** Reads and waits without activating or cancelling Session execution. */
export class StateController {
    ctx;
    historyController;
    /**
     * @param ctx - live Session and pending Remote event services.
     * @param historyController - passive read entry used for cold state and its bounded output.
     */
    constructor(ctx, historyController) {
        this.ctx = ctx;
        this.historyController = historyController;
    }
    /**
     * Read passive execution and pending-input state.
     * @param request - Session identity.
     * @param signal - read cancellation.
     * @returns current passive state.
     */
    async state(request, signal) {
        return this.readState(request.sessionId, signal);
    }
    /**
     * Observe complete passive state replacements.
     * @param request - Session identity.
     * @param signal - observation cancellation.
     * @returns complete state replacements, starting with the current state.
     */
    async *observeState(request, signal) {
        const env_1 = { stack: [], error: void 0, hasError: false };
        try {
            const listener = __addDisposableResource(env_1, this.listen(request.sessionId, signal), false);
            for await (const { state } of listener.observeStates())
                yield state;
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
     * Wait for quiescence, pending input, or the deadline without cancelling execution.
     * @param request - Session identity, timeout and assistant output window.
     * @param signal - cancellation of this wait only, never of Agent execution.
     * @returns the stopping reason and last complete state; no state before timeout is an error.
     */
    async wait(request, signal) {
        const env_2 = { stack: [], error: void 0, hasError: false };
        try {
            const timeoutMs = request.timeoutMs ?? DEFAULT_WAIT_TIMEOUT_MS;
            const afterSeq = request.afterSeq ?? -1;
            const maxMessages = request.maxMessages ?? DEFAULT_MAX_MESSAGES;
            if (!Number.isSafeInteger(afterSeq) || afterSeq < -1 || Object.is(afterSeq, -0)) {
                throw new RemoteError('gateway/bad-request', 'afterSeq must be an integer greater than or equal to -1', {});
            }
            if (!Number.isSafeInteger(maxMessages) || maxMessages <= 0) {
                throw new RemoteError('gateway/bad-request', 'maxMessages must be a positive safe integer', {});
            }
            if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0 || timeoutMs > MAX_TIMER_DELAY_MS) {
                throw new RemoteError('gateway/bad-request', 'timeoutMs must be a positive supported timer delay', {});
            }
            const timer = __addDisposableResource(env_2, deadline(signal, timeoutMs, 'session/wait-timeout'), false);
            const listener = __addDisposableResource(env_2, this.listen(request.sessionId, timer.signal), false);
            const sessionId = request.sessionId;
            const statesGenerator = listener.observeStates(1_000);
            let nextStatePromise = statesGenerator.next();
            let currentState;
            let reason = 'timeout';
            try {
                for (;;) {
                    const observedState = await nextStatePromise;
                    if (observedState.done) {
                        timer.signal.throwIfAborted();
                        throw new RemoteError('session/observation-ended', 'Session observation ended before a stopping state', {});
                    }
                    nextStatePromise = statesGenerator.next();
                    // Observe rejection on immediate exits without replacing the original demand.
                    void nextStatePromise.catch(() => undefined);
                    // Only a state the listener still confirms may drive this wait; an invalidated
                    // sample never becomes the state a timeout reports.
                    if (!listener.isCurrent(observedState.value.revision))
                        continue;
                    currentState = observedState.value.state;
                    const action = computeWaitAction(currentState);
                    if (action === 'continue')
                        continue;
                    if (action === 'input-required' || action === 'idle') {
                        reason = action;
                        break;
                    }
                    const agent = this.ctx.agents.get(sessionId);
                    if (agent === undefined || this.ctx.sessions.get(sessionId) !== agent.session)
                        continue;
                    if (await waitForIdleOrStateChange(agent, nextStatePromise) === 'state-changed')
                        continue;
                    if (!listener.isCurrent(observedState.value.revision) || this.ctx.agents.get(sessionId) !== agent
                        || this.ctx.sessions.get(sessionId) !== agent.session)
                        continue;
                    reason = 'idle';
                    break;
                }
            }
            catch (error) {
                // A caller cancellation is never a wait outcome; the deadline absorbs only the read
                // it cancelled, and any other failure keeps its own error.
                if (signal.aborted)
                    throw signal.reason;
                if (!expired(timer.signal, error))
                    throw error;
            }
            finally {
                // Wake/cancel a pending read before asking the generator to return.
                listener[Symbol.dispose]();
                await nextStatePromise.catch(() => undefined);
                await statesGenerator.return(undefined);
            }
            signal.throwIfAborted();
            if (currentState === undefined)
                throw new RemoteError('session/wait-timeout', 'No Session state arrived before the wait deadline', {});
            const output = await this.historyController.assistantOutput(sessionId, afterSeq, currentState.cursor, maxMessages, signal);
            return { reason, state: currentState, output };
        }
        catch (e_2) {
            env_2.error = e_2;
            env_2.hasError = true;
        }
        finally {
            __disposeResources(env_2);
        }
    }
    listen(sessionId, signal) {
        return new StateListener(this.ctx, sessionId, signal, readSignal => this.readState(sessionId, readSignal));
    }
    /**
     * @param sessionId - Session to sample.
     * @param signal - read cancellation.
     * @param retried - whether a live source already redirected this read once.
     * @returns one passive state sample.
     */
    async readState(sessionId, signal, retried = false) {
        signal.throwIfAborted();
        const agent = this.ctx.agents.get(sessionId);
        if (agent !== undefined) {
            if (agent.session.header.cwd === undefined)
                rejectNotFound({ kind: 'session', sessionId });
            return {
                cursor: agent.session.seq - 1,
                active: true,
                running: agent.status === 'running',
                queue: queueItems(agent),
                pendingInput: this.pendingInput(agent.session.id),
            };
        }
        const state = await this.historyController.usingObservation({ sessionId, projection: false }, signal, (observation) => {
            // A live source means the Session attached during this read; report its live state
            // rather than a live cursor paired with an inactive sample.
            if (!retried && observation.source === 'live')
                return undefined;
            if (observation.header.cwd === undefined)
                rejectNotFound({ kind: 'session', sessionId });
            return {
                cursor: observation.cursor,
                active: false,
                running: false,
                queue: null,
                pendingInput: this.pendingInput(observation.header.id),
            };
        });
        signal.throwIfAborted();
        return state ?? this.readState(sessionId, signal, true);
    }
    pendingInput(sessionId) {
        const kinds = new Set();
        for (const { event, agentId } of this.ctx.typertGateway.getPendingRemoteEvents()) {
            if (String(agentId) !== sessionId)
                continue;
            kinds.add(event === 'approval/request' ? 'approval' : event === 'user-questions/request' ? 'question' : 'other');
        }
        return [...kinds];
    }
}
/**
 * @param signal - wait deadline signal.
 * @param error - failure raised by the in-flight read.
 * @returns whether the deadline expired and cancelled that read.
 */
function expired(signal, error) {
    const reason = timeoutOf(signal, 'session/wait-timeout');
    if (reason === undefined)
        return false;
    // Cancellation carries the deadline reason along its cause chain; any other failure does not.
    let current = error;
    for (let depth = 0; depth < 4 && current instanceof Error; depth++) {
        if (current === reason || current.cause === reason)
            return true;
        current = current.cause;
    }
    return error === reason;
}
/** Choose the next wait action; check-idle requires the caller's whenIdle and identity checks. */
function computeWaitAction(state) {
    if (state.pendingInput.length > 0)
        return 'input-required';
    if (state.running)
        return 'continue';
    if (!state.active)
        return 'idle';
    return 'check-idle';
}
/** Wait for either driver quiescence or the already-armed next state; preserve rejections from both. */
function waitForIdleOrStateChange(agent, nextStatePromise) {
    return Promise.race([
        agent.whenIdle().then(() => 'idle'),
        nextStatePromise.then(() => 'state-changed'),
    ]);
}
function queueItems(agent) {
    const item = (message, placement) => ({
        id: message.id,
        placement,
        ...message.source.kind === 'user' && 'rpcId' in message.source ? { rpcId: message.source.rpcId } : {},
        content: message.content,
    });
    return [
        ...agent.inbox.nextTurn.map(message => item(message, 'queued')),
        ...agent.inbox.nextStep.map(message => item(message, message.source.kind === 'user' ? 'steering' : 'context')),
    ];
}
//# sourceMappingURL=controller.js.map