/**
 * Contained lifecycle notifications for subagents.
 *
 * The service owns scoped dispatch and listener containment; the manager
 * supplies event payloads from each activation.
 *
 * @module @deepseek-ai/dsh-subagent/lifecycle
 */
import { foldConsumedWork } from '@deepseek-ai/dsh-agent';
/**
 * Build the contained lifecycle emitter this seam publishes every edge through.
 * Every listener is independently contained: a synchronous throw or a rejected
 * returned promise is logged without starving peer listeners, changing the run,
 * or — for provider removal, which fires from a disposer — breaking teardown.
 * @param ctx - the service's own context, owning dispatch and the logger.
 * @param carrier - resolve the scoped dispatch carrier for one subagent tree root.
 * @returns the emitter the manager and provider registry publish through.
 */
export function createLifecycleEmitter(ctx, carrier) {
    return (name, info, parent) => {
        const dispatchArgs = parent === undefined
            ? [name, info]
            : [carrier(parent), name, info];
        for (const callback of ctx.events.dispatch('emit', dispatchArgs)) {
            try {
                const returned = callback(info);
                void Promise.resolve(returned).catch((error) => {
                    ctx.logger.warn(`subagent: ${name} listener rejected: ${renderThrown(error)}`);
                });
            }
            catch (error) {
                ctx.logger.warn(`subagent: ${name} listener threw: ${renderThrown(error)}`);
            }
        }
    };
}
/**
 * Why this child's epoch ended, for the terminal lifecycle edge and the
 * manager's own parent delivery. The child's own log is authoritative:
 * teardown succeeding says nothing about whether the model errored, hit its
 * token ceiling, or was cancelled, so deriving the reason from disposal would
 * report failed work as completed.
 *
 * {@link foldConsumedWork} supplies both halves the raw turn sequence cannot:
 * which turn accounts for the work this epoch consumed, and whether accepted
 * work was cancelled after it without any turn opening over it. A recorded
 * failure still wins over a cancellation — stopping a child that had already
 * failed does not turn its failure into a cancellation.
 * @param events - this epoch's own event suffix.
 * @returns its terminal stop reason; `completed` only for an epoch that both
 *   closed cleanly and had nothing left to run.
 */
export function epochStopReason(events) {
    const { end, droppedUnrun } = foldConsumedWork(events);
    switch (end?.data.reason.kind) {
        case 'max-tokens':
            return 'max-tokens';
        case 'aborted':
        case 'interrupted':
            return 'aborted';
        case 'error':
            return 'error';
        // A pre-step rejection — a hook deny, a policy plugin — discarded input
        // this epoch had claimed: the work was declined, not done.
        case 'blocked':
            return 'refusal';
        // A clean ending and no accounting turn at all share one rule: the epoch
        // finished what it was given unless a cancelled queue says otherwise.
        case undefined:
        case 'completed':
            return droppedUnrun ? 'aborted' : 'completed';
        /* v8 ignore next 4 -- `forked` appears only in constructor seed history, while
         * this function reads an epoch-owned suffix. `TurnEndReason` is merge-extensible,
         * so a backend-added variant cannot be listed; treating an unnameable reason as
         * success would report failed work as completed. */
        default:
            return 'error';
    }
}
/** Render any listener-thrown value without letting coercion escape containment. */
function renderThrown(value) {
    try {
        return value instanceof Error ? `${value.name}: ${value.message}` : String(value);
    }
    catch {
        return '<unrenderable thrown value>';
    }
}
//# sourceMappingURL=lifecycle.js.map