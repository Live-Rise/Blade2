/** Saved delivery retention and identity-based pagination without Session reads. */
import type { ScheduleTask } from './storage.ts';
import type { DeliveryRetentionBounds, ScheduleDeliveryHistoryRequest, ScheduleDeliveryHistoryResult, ScheduleDeliveryReceipt } from './types.ts';
/**
 * Prepare one real inbox acknowledgment for the same task write as its status and target.
 * Prunes to the configured window and record cap here, on the only write path, so a task record does not grow
 * with every acknowledgment.
 * @param task - Task supplying the immutable sent prompt and retained history.
 * @param receipt - Acknowledgment obtained after successful Session flush.
 * @param bounds - Configured retention window and record cap for the appended history.
 * @returns Receipt and retained history; the caller publishes them only after task persistence.
 */
export declare function appendDelivery(task: ScheduleTask, receipt: ScheduleDeliveryReceipt, bounds: DeliveryRetentionBounds): Required<Pick<ScheduleTask, 'lastDelivery' | 'deliveryHistory'>>;
/**
 * Read one newest-first page in append order, independent of wall-clock ordering.
 * @param task - Task already selected by the caller.
 * @param request - Validated explicit page size and optional exclusive message cursor.
 * @param retention - Current configured limits shared with the delivery writer.
 * @returns Copied delivery records or a cursor-not-found result.
 */
export declare function deliveryHistoryPage(task: ScheduleTask, request: ScheduleDeliveryHistoryRequest, retention: DeliveryRetentionBounds): ScheduleDeliveryHistoryResult;
//# sourceMappingURL=delivery-history.d.ts.map