import type { ContextFormed } from '@deepseek-ai/dsh-llm';
declare module '@deepseek-ai/dsh-llm' {
    interface MessageSourceMap {
        'schedule': {
            kind: 'schedule';
        } & ContextFormed;
    }
}
import { type SessionId as SessionIdType } from '@deepseek-ai/dsh-session';
import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { ScheduleTask } from './storage.ts';
import type { DeliveryRetentionBounds, ScheduleId } from './types.ts';
/** Largest delay Node timers represent without clamping. */
export declare const MAX_TIMER_DELAY_MS = 2147483647;
/** Fixed per-task delivery floor interval, timed or manual, of a target other than `new`; it matches the shortest recurrence spacing. */
export declare const MIN_DELIVERY_INTERVAL_MS: number;
/** Resolved deployment limits of one runtime. */
export interface ScheduleRuntimeLimits {
    /** Retention window and record cap applied on each delivery commit. */
    readonly retention: DeliveryRetentionBounds;
    /** Delivery floor interval of one task whose every delivery creates a Session. */
    readonly newSessionIntervalMs: number;
}
/** Owns at most one timer across recomputations; delivery and management share the serialized operation. */
export declare class ScheduleRuntime {
    private readonly ctx;
    private readonly tasks;
    private readonly transact;
    private readonly commit;
    private readonly changed;
    private readonly limits;
    private readonly resolveAgent;
    private timer;
    private running;
    private readonly executing;
    private stopping;
    private requested;
    /**
     * @param ctx - Host services used to resume and enqueue.
     * @param tasks - Current durable tasks.
     * @param transact - Serialize delivery against management writes.
     * @param commit - Persist one task value and publish its change; delivery commits status, target, and receipt together.
     * @param changed - Publish a change that has no task write, such as an execution reaching whole-agent idle.
     * @param limits - Resolved retention and new-Session admission limits.
     * @param resolveAgent - Host fallback after embedding activation handlers delegate.
     */
    constructor(ctx: Context, tasks: () => readonly ScheduleTask[], transact: <T>(work: () => Promise<T>) => Promise<T>, commit: (task: ScheduleTask) => Promise<void>, changed: () => void, limits: ScheduleRuntimeLimits, resolveAgent: (sessionId: SessionIdType) => Promise<Agent>);
    /**
     * Recompute the nearest obligation after startup or a durable change.
     * Dispatch failures are logged; refused admission does not retry automatically.
     */
    requestDrive(): void;
    /** Stop the timer and drain an accepted delivery before storage closes. */
    dispose(): Promise<void>;
    /**
     * Queue one manual execution without consuming or moving its scheduled occurrence.
     * @param id - Canonical task identity resolved again inside the shared transaction.
     * @param signal - Cancellation checked after entering the transaction and before delivery.
     * @returns Admission and task state after durable Session inbox acknowledgement, or undefined when deleted first.
     */
    runNow(id: ScheduleId, signal?: AbortSignal): Promise<{
        readonly task: ScheduleTask;
        readonly admitted: boolean;
    } | undefined>;
    /**
     * Whether a queued or running execution holds this task's slot.
     * @param id - Task identity.
     * @returns Whether a delivery to the task's reused target Session has not yet reached whole-agent idle; a `new` target holds no slot.
     */
    isExecutionActive(id: ScheduleId): boolean;
    /**
     * End of this task's current minimum delivery interval.
     * @param task - Current durable task state.
     * @returns Epoch milliseconds while the interval remains in the future.
     */
    deliveryFloorEnd(task: ScheduleTask): number | undefined;
    private clearTimer;
    private drive;
    /**
     * Deliver one due group in one message and advance each committed member.
     * @param group - One task, or the due recurring tasks of one target Session.
     * @param batch - Whether the message uses the batch framing.
     * @returns Members that remain due after a failure; the timer skips them until the next drive request.
     */
    private deliverDue;
    /** Earliest wall-clock instant at which this task may admit another delivery. */
    private nextAdmissionAt;
    /** Whether the Workspace registry lists this Session as archived; without a registry, no Session is archived. */
    private archived;
    /**
     * Reconstruct a still-queued last delivery after a Host restart without storing model completion state.
     * @param task - Task whose last delivered message may still wait in its target inbox.
     * @returns Whether the task holds its slot for that delivery; a woken reminder of a `new` target holds none.
     */
    private recoverOutstandingExecution;
    /**
     * Enqueue one reminder and wait for Session persistence to acknowledge it.
     * The held tasks hold their slots from the enqueue; a failure releases them.
     */
    private send;
    /** Release each uncommitted held slot now and each committed one at the target Agent's whole-agent idle boundary. */
    private settle;
    /** Release admitted executions at the target Agent's whole-agent idle boundary, then recompute the timer. */
    private watchExecution;
    /** Apply one receipt to the target, involved Sessions, and history while keeping the scheduled occurrence. */
    private delivered;
    /** Apply one timed receipt and advance the rule that owns dispatch to its next target. */
    private advanced;
    /**
     * Resolve the delivery target and its live Agent.
     * A `new` target reserves its Session identity in the task before creation, so a failed delivery retries the same
     * Session instead of creating another. A stored reserved identity is resolved first and created only when no Session
     * has it, because the Session controller creates a Session with a different id when the named Session has already
     * started a turn. A created Session joins the Workspace registered at `create.cwd` and takes the task title; a `new`
     * target's title adds its launch time in the task's time zone. Neither step blocks delivery.
     */
    private resolveTarget;
    /** Resolve the live Agent of a reserved Session; `undefined` means that no Session has its id yet. */
    private reservedAgent;
    /** Resolve through the embedding's activation owner, with the injected Web controller as fallback. */
    private activate;
    /** Create a reserved Session with the target's working directory and Agent preset, then resolve its live Agent. */
    private createAgent;
    /** Attach a created Session to the Workspace registered at its directory; a failure is logged and does not stop delivery. */
    private joinWorkspace;
    /** Name a created Session that has no title yet; a failure is logged and does not stop delivery. */
    private nameSession;
}
//# sourceMappingURL=runtime.d.ts.map