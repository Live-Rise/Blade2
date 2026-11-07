import type { ScheduleId, SchedulePatch, ScheduleRecord, ScheduleTiming } from './types.ts';
/**
 * Construct a current record with one authoritative timing kind and a future committed target.
 * @param id - Host-generated task identity.
 * @param title - Required user-facing task title.
 * @param prompt - Non-empty instruction delivered at each occurrence.
 * @param value - Exactly one typed timing selector.
 * @param now - Explicit acceptance time in epoch milliseconds.
 * @returns A validated record with its first future occurrence.
 */
export declare function createScheduleRecord(id: ScheduleId, title: string, prompt: string, value: ScheduleTiming, now: number): ScheduleRecord;
/**
 * Keep unchanged timing and committed targets, or re-arm through the same constructor as creation.
 * @param current - Stored record before the edit.
 * @param patch - Descriptive fields and optional typed timing replacement.
 * @param now - Explicit edit acceptance time in epoch milliseconds.
 * @param resume - Whether to find a future occurrence for unchanged timing.
 * @returns The retained or revised authoritative timing record.
 */
export declare function updateScheduleRecord(current: ScheduleRecord, patch: SchedulePatch, now: number, resume: boolean): ScheduleRecord;
//# sourceMappingURL=update.d.ts.map