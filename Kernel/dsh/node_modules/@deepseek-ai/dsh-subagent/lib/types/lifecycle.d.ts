/**
 * Contained lifecycle notifications for subagents.
 *
 * The service owns scoped dispatch and listener containment; the manager
 * supplies event payloads from each activation.
 *
 * @module @deepseek-ai/dsh-subagent/lifecycle
 */
import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { SessionEvent } from '@deepseek-ai/dsh-session';
import type { SubagentResult, SubagentRunEndInfo, SubagentRunInfo } from './types.ts';
/**
 * Publish one notification with per-listener exception containment. Run edges
 * carry the subagent tree root that keys scoped dispatch. Provider removal
 * reaches all scopes.
 *
 * The service owns this closure because scoped dispatch keys its carrier by the
 * exact service instance, whose own context filter composes into the carrier;
 * a narrowed stand-in would silently change scope filtering.
 */
export type LifecycleEmitter = {
    (name: 'subagent/start', info: SubagentRunInfo, parent: Agent): void;
    (name: 'subagent/end', info: SubagentRunEndInfo, parent: Agent): void;
    (name: 'subagent/provider-removed', info: string): void;
};
/**
 * Build the contained lifecycle emitter this seam publishes every edge through.
 * Every listener is independently contained: a synchronous throw or a rejected
 * returned promise is logged without starving peer listeners, changing the run,
 * or — for provider removal, which fires from a disposer — breaking teardown.
 * @param ctx - the service's own context, owning dispatch and the logger.
 * @param carrier - resolve the scoped dispatch carrier for one subagent tree root.
 * @returns the emitter the manager and provider registry publish through.
 */
export declare function createLifecycleEmitter(ctx: Context, carrier: (parent: Agent) => object): LifecycleEmitter;
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
export declare function epochStopReason(events: readonly SessionEvent[]): SubagentResult['stopReason'];
//# sourceMappingURL=lifecycle.d.ts.map