/** Catalog vocabulary shared by the Session surfaces and Host task management. */
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { ScheduleCatalogEntry, RRuleScheduleRecord } from '@deepseek-ai/dsh-schedule/client';
/** Earliest scheduled delivery after applying the Host's current admission interval.
 * @param task - Catalog entry with its scheduled occurrence and optional delivery floor.
 * @returns The later of the occurrence and delivery floor, without changing the stored rule.
 */
export declare function taskNextRunAt(task: Pick<ScheduleCatalogEntry, 'scheduledAt' | 'deliveryFloorEndsAt'>): string;
/** The reusable target Session, or the latest per-execution Session when one exists.
 * @param task - Task whose conversation link is shown.
 * @returns Destination identity, if a delivery or reservation names one.
 */
export declare function taskSessionId(task: Pick<ScheduleCatalogEntry, 'target' | 'lastDelivery'>): SessionId | undefined;
/** Whether one Session should expose this task in its existing clock entry.
 * @param task - Catalog entry being inspected.
 * @param sessionId - Session owning the entry.
 * @returns Whether the task targets or involves this Session.
 */
export declare function taskInvolvesSession(task: ScheduleCatalogEntry, sessionId: SessionId): boolean;
/** RRULE fields read for presentation; dispatch remains owned by the Host.
 * @param rrule - Host-validated recurrence text.
 * @returns Fields from its RRULE content line.
 */
export declare function ruleFields(rrule: string): ReadonlyMap<string, string>;
/** Guided timing choices; rules that cannot round-trip through the form stay custom.
 * @param task - Authoritative timing record.
 * @returns Guided choice, or custom for an unsupported rule.
 */
export declare function hostFrequencyKind(task: RRuleScheduleRecord): 'once' | 'minutely' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'custom';
//# sourceMappingURL=task-record.d.ts.map