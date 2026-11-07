import z from '@deepseek-ai/schemastery';
import { Context, Service } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import { type SessionId } from '@deepseek-ai/dsh-session';
import type { ScheduleCatalogEntry, ScheduleCreateRequest, ScheduleDeleteInactiveResult, ScheduleDeliveryHistoryRequest, ScheduleDeliveryHistoryResult, ScheduleEditRequest, ScheduleEditResult, ScheduleListRequest, ScheduleListResult, ScheduleRunRequest, ScheduleRunResult, ScheduleViewRequest, ScheduleViewResult } from './types.ts';
export type * from './client.ts';
export { scheduleDomain } from './storage.ts';
export type { ScheduleTask } from './storage.ts';
export type { RecurringOccurrence } from './domain.ts';
export { formatScheduleLocalTime } from './domain.ts';
export { MIN_EVERY_INTERVAL_SECONDS, MAX_TITLE_LENGTH, REQUIRED_TITLE_MESSAGE, ScheduleId, ScheduleInputError, ScheduleLogError, canonicalizeCronExpression, createAfterScheduleRecord, createAtScheduleRecord, createEveryScheduleRecord, createDailyScheduleRecord, createWeeklyScheduleRecord, createCronScheduleRecord, decodeScheduleRecord, isRecurringScheduleRecord, normalizeWeekdays, parseCronInput, parseWeeklyInput, renderReminderFraming, renderRecurringReminderBatchFraming, resolveEveryOccurrence, resolveDailyOccurrence, resolveWeeklyOccurrence, resolveCronOccurrence, resolveRecurringOccurrence, scheduleTitle, weeklyTime, } from './domain.ts';
declare module '@deepseek-ai/cordis' {
    interface Events {
        /**
         * Restore a reminder's Session through its embedding's owning controller.
         * Call next() to use the Web Session controller.
         * @param sessionId - Original Session receiving the reminder.
         * @mode waterfall
         */
        'schedule/resolve-agent'(sessionId: SessionId, next: () => Promise<Agent>): Promise<Agent>;
    }
    interface Context {
        /** Durable Host-wide reminder management. */
        schedule: ScheduleService;
    }
}
/** Configuration for the Host Schedule domain. */
export interface Config {
    /**
     * Delivery-history window retained per task, in days; omission defaults to 30.
     * Pruning happens when an acknowledgment is appended, and `lastDelivery` is always retained.
     */
    deliveryHistoryDays?: number;
    /**
     * Retained delivery records per task; omission defaults to 200. The older of this
     * cap and the window wins, and the newest records survive.
     */
    deliveryHistoryRecords?: number;
    /**
     * Shortest spacing, in seconds, from the occurrence of one delivery to the next delivery of one task whose target
     * creates a Session at every delivery; omission defaults to 60. Timed and manual deliveries both wait for it.
     */
    newSessionIntervalSeconds?: number;
}
/** Page size of a task list request that states none. */
export declare const DEFAULT_LIST_LIMIT = 100;
/** Largest page size of one task list request. */
export declare const MAX_LIST_LIMIT = 500;
/**
 * Shared management service; reads and management writes never activate a Session, and only delivery creates or
 * resumes one.
 *
 * `sessionPersistence` supplies header-only target validation and the `session/flush`
 * listener that acknowledges durable inbox delivery through `ctx.sessions.flush()`.
 */
export declare class ScheduleService extends TypertRemoteService {
    static inject: string[];
    static Config: z<Config>;
    /** Resolved retention and admission limits shared with the runtime that appends acknowledgments. */
    private readonly limits;
    private readonly ready;
    private readonly initialized;
    private chain;
    private notifications;
    private runtime;
    private resolveWebAgent;
    private stopping;
    /**
     * @param ctx - Host services owning storage, dispatch, and Session restoration.
     * @param config - Validated delivery-history retention and new-Session admission configuration.
     */
    constructor(ctx: Context, config: Config);
    [Service.init](): Promise<void>;
    /** Read effective delivery limits without loading or activating a task.
     * @returns The configured minimum spacing between deliveries that each create a Session, in seconds.
     */
    deliveryLimits(): {
        readonly newSessionIntervalSeconds: number;
    };
    /**
     * Create one task with a Host-generated identity for a human Remote caller.
     * @param request - Title, prompt, recurrence, and delivery destination.
     * @param signal - Cancellation checked before the durable write begins, including after FIFO waits.
     * @returns The stored task. Cancellation does not roll back an in-flight write.
     */
    remoteCreate(request: ScheduleCreateRequest, signal?: AbortSignal): Promise<ScheduleCatalogEntry>;
    /**
     * Create one task and record the acting Agent's Session as involved; the Remote form omits that Session.
     *
     * The record is built from the clock reading taken before the request joins the serialized queue. A `session`
     * target must exist and must not belong to a delegated child; a `new_once` target reserves its Session identity.
     * @param request - Title, prompt, recurrence, and delivery destination.
     * @param actorSessionId - Session of the acting Agent, or undefined for a human caller.
     * @param signal - Cancellation checked before the durable write begins, including after FIFO waits.
     * @returns The stored task. Cancellation does not roll back an in-flight write.
     */
    create(request: ScheduleCreateRequest, actorSessionId?: SessionId, signal?: AbortSignal): Promise<ScheduleCatalogEntry>;
    /**
     * Read one page of Host tasks without activating target Sessions.
     * @param request - Optional activity filter, page size, and offset.
     * @returns The requested page and total matching task count.
     */
    list(request?: ScheduleListRequest): Promise<ScheduleListResult>;
    /**
     * Remote form of the shared Host-wide list.
     * @param request - Optional activity filter, page size, and offset.
     * @returns The requested page and total matching task count.
     */
    remoteList(request: ScheduleListRequest): Promise<ScheduleListResult>;
    /**
     * Read every Host task without a page bound or Session activation.
     * @returns All tasks, active first, with target and execution metadata.
     */
    catalog(): Promise<ScheduleCatalogEntry[]>;
    /**
     * Read one task without activating its target Sessions.
     * @param request - Task identity.
     * @returns The task, or a non-mutating miss.
     */
    view(request: ScheduleViewRequest): Promise<ScheduleViewResult>;
    /**
     * Read one page of saved deliveries without activating or reading a target Session.
     * @param request - Task identity, explicit page size, and optional exclusive cursor.
     * @returns Newest-first saved deliveries, or a task/cursor lookup failure.
     */
    history(request: ScheduleDeliveryHistoryRequest): Promise<ScheduleDeliveryHistoryResult>;
    /**
     * Remote form of the target-based {@link ScheduleService.history}; an invalid limit rejects with
     * `schedule/input-invalid`.
     * @param request - Task identity, explicit limit, and optional exclusive message cursor.
     * @returns Newest-first deliveries in append order, or a task/cursor lookup failure.
     */
    remoteHistory(request: ScheduleDeliveryHistoryRequest): Promise<ScheduleDeliveryHistoryResult>;
    /**
     * Deliver one task now without consuming or moving its scheduled occurrence; an archived destination Session rejects
     * with `schedule/target-archived`.
     * @param request - Task identity.
     * @param signal - Cancellation checked after entering the delivery transaction and before delivery.
     * @returns Task state after Session persistence acknowledged the delivery, `schedule_execution_pending` while
     * another execution of the task is queued, running, or inside its delivery floor, or a non-mutating miss.
     */
    run(request: ScheduleRunRequest, signal?: AbortSignal): Promise<ScheduleRunResult>;
    /**
     * Delete every ended Host task, leaving active tasks armed.
     *
     * Ended tasks never schedule another delivery, so the removal changes no pending
     * target and no timer is recomputed. Rows are enumerated and removed in one FIFO
     * slot, so a delivery ordered behind this call ends its task after the
     * enumeration rather than during it. The batch is one domain write: a normal
     * failure rejects with every listed row still stored, while a backend
     * `unknown-state` failure may have removed rows and quarantines the Host
     * domain, so later schedule operations reject until the domain is reopened.
     * A successful nonempty batch emits `schedule/changed` without Agent notices.
     * A rejected batch announces `schedule/changed` because it does not establish
     * which rows landed, and clients then re-read the catalog.
     * @returns Count of removed rows; zero when the Host held no ended task.
     */
    deleteInactive(): Promise<ScheduleDeleteInactiveResult>;
    /**
     * Update or delete one task for a human Remote caller; an invalid patch rejects with `schedule/input-invalid`.
     * @param request - Task identity and one update or delete operation.
     * @param signal - Cancellation checked before the durable write begins, including after FIFO waits.
     * @returns The updated task, the deletion outcome, or a non-mutating miss or conflict.
     */
    remoteEdit(request: ScheduleEditRequest, signal?: AbortSignal): Promise<ScheduleEditResult>;
    /**
     * Update or delete one task and record the acting Agent's Session as involved; the Remote form omits that Session.
     *
     * An update applies every patch field in one write and recomputes the next occurrence when the rule changes or
     * the task resumes. A supplied `expectedUpdatedAt` that differs from the stored value returns `schedule_conflict`.
     * @param request - Task identity and one update or delete operation.
     * @param actorSessionId - Session of the acting Agent, or undefined for a human caller.
     * @param signal - Cancellation checked before the durable write begins, including after FIFO waits.
     * @returns The updated task, the deletion outcome, or a non-mutating miss or conflict.
     */
    edit(request: ScheduleEditRequest, actorSessionId?: SessionId, signal?: AbortSignal): Promise<ScheduleEditResult>;
    /**
     * Dispatch one post-commit `schedule/changed` notification, containing
     * synchronous listener failures: every call site emits only after its durable
     * task write landed, so a throwing listener must not reject the caller or
     * skip the following `requestDrive()`.
     */
    private emitChanged;
    private getDomain;
    /**
     * Resolve a requested destination without activating a Session.
     *
     * A `session` target must exist and must not belong to a delegated child: a live Agent answers through
     * {@link delegationDepthOf}, and a stored Session answers through the delegation depth in its saved header.
     * @param request - Requested destination.
     * @param signal - Cancellation of the stored Session header read.
     * @returns Stored destination; a `new_once` target reserves its Session identity here.
     */
    private resolveTarget;
    /** Check arming prerequisites without activating a Session; repeated inside the mutation FIFO. */
    private validateTarget;
    /** New destinations require a provider before a task can be armed. */
    private requireSessionCreation;
    /** Every stored task as a catalog entry, in {@link compareEntries} order. */
    private entries;
    private entry;
    /**
     * Tell each involved Session's live Agent about one committed change, except the acting Session.
     * A stored Session stays inactive; it reads the current task state when it next lists its tasks.
     */
    private notify;
    /**
     * Queue one change notice behind earlier notices without adding Session I/O to the management operation.
     * Callers queue inside the serialized operation, so teardown, which drains that queue first, cannot overtake it.
     */
    private queueNotification;
    /**
     * Remove every active task that delivers to one Session, inside the queue the tools
     * use.
     *
     * Enumerating and deleting in one queue slot is what makes an archive stop
     * ordered behind a create whose write is still in flight: a stop that read the
     * table outside the queue could miss a row the create was about to commit and
     * leave an armed reminder behind. Re-entering the public `edit()` from here
     * would deadlock on this queue, so the rows are removed directly.
     * @param sessionId - Session whose active Host tasks must stop.
     */
    private stopSessionTasks;
    /**
     * Remove rows as one durable batch inside the current queue slot.
     *
     * The batch is one domain write: on success every present row is removed and
     * announced. Any rejection announces `schedule/changed` because it does not
     * establish which rows landed; a normal failure leaves every row stored, and
     * an `unknown-state` failure may have removed some. A rejection reason that
     * carries no Error is wrapped with the operation label.
     * @param ids - rows to remove.
     * @param label - failure message prefix naming the operation that owned them.
     * @param rearm - whether the rows were armed, so the pending timer must be recomputed.
     * @returns the number of rows this call removed.
     */
    private removeRows;
    private serialize;
}
export default ScheduleService;
//# sourceMappingURL=index.d.ts.map