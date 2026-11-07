import { z } from 'zod';
import { SessionId } from '@deepseek-ai/dsh-session';
import { MessageId } from '@deepseek-ai/dsh-llm/brand';
import type { ScheduleDeliveryRecord, ScheduleId, ScheduleTarget } from './types.ts';
/**
 * Version-2 task schema accepting version-1 rows. New metadata is optional on disk. An explicit target
 * ignores the former Session binding; a missing target uses that binding or a stable create-once destination.
 * Normal decoding does not write. The next edit or delivery writes this current shape without `sessionId`.
 */
export declare const scheduleTaskSchema: z.ZodPipe<z.ZodObject<{
    record: z.ZodPipe<z.ZodUnknown, z.ZodTransform<import("./types.ts").AfterScheduleRecord | import("./types.ts").AtScheduleRecord | import("./types.ts").EveryScheduleRecord | import("./types.ts").DailyScheduleRecord | import("./types.ts").WeeklyScheduleRecord | import("./types.ts").CronScheduleRecord | import("./types.ts").RRuleScheduleRecord, unknown>>;
    sessionId: z.ZodOptional<z.ZodUnknown>;
    target: z.ZodOptional<z.ZodDiscriminatedUnion<[z.ZodObject<{
        kind: z.ZodLiteral<"session">;
        sessionId: z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>;
    }, z.core.$strict>, z.ZodObject<{
        kind: z.ZodLiteral<"new_once">;
        sessionId: z.ZodOptional<z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>>;
        create: z.ZodDefault<z.ZodObject<{
            cwd: z.ZodExactOptional<z.ZodString>;
            agentPreset: z.ZodExactOptional<z.ZodString>;
        }, z.core.$strict>>;
    }, z.core.$strict>, z.ZodObject<{
        kind: z.ZodLiteral<"new">;
        create: z.ZodDefault<z.ZodObject<{
            cwd: z.ZodExactOptional<z.ZodString>;
            agentPreset: z.ZodExactOptional<z.ZodString>;
        }, z.core.$strict>>;
    }, z.core.$strict>], "kind">>;
    involvedSessionIds: z.ZodOptional<z.ZodArray<z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>>>;
    status: z.ZodDefault<z.ZodEnum<{
        active: "active";
        paused: "paused";
        ended: "ended";
        inactive: "inactive";
    }>>;
    createdAt: z.ZodOptional<z.ZodISODateTime>;
    updatedAt: z.ZodOptional<z.ZodISODateTime>;
    lastDelivery: z.ZodOptional<z.ZodObject<{
        scheduledAt: z.ZodISODateTime;
        deliveredAt: z.ZodISODateTime;
        messageId: z.ZodPipe<z.ZodString, z.ZodTransform<MessageId, string>>;
        targetSessionId: z.ZodOptional<z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>>;
    }, z.core.$strict>>;
    deliveryHistory: z.ZodOptional<z.ZodObject<{
        records: z.ZodArray<z.ZodObject<{
            scheduledAt: z.ZodISODateTime;
            deliveredAt: z.ZodISODateTime;
            messageId: z.ZodPipe<z.ZodString, z.ZodTransform<MessageId, string>>;
            targetSessionId: z.ZodOptional<z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>>;
            prompt: z.ZodExactOptional<z.ZodString>;
        }, z.core.$strict>>;
        earlierRecordsUnavailable: z.ZodBoolean;
        earlierRecordsPruned: z.ZodDefault<z.ZodBoolean>;
    }, z.core.$strict>>;
    pendingDelivery: z.ZodOptional<z.ZodObject<{
        targetSessionId: z.ZodPipe<z.ZodString, z.ZodTransform<SessionId, string>>;
    }, z.core.$strict>>;
}, z.core.$strict>, z.ZodTransform<{
    deliveryHistory: {
        records: ScheduleDeliveryRecord[];
        earlierRecordsUnavailable: boolean;
        earlierRecordsPruned: boolean;
    };
    lastDelivery?: {
        scheduledAt: string;
        deliveredAt: string;
        messageId: MessageId;
        targetSessionId?: SessionId | undefined;
    } & {
        targetSessionId: SessionId;
    };
    target: ScheduleTarget;
    status: "active" | "paused" | "ended";
    involvedSessionIds: SessionId[];
    record: import("./types.ts").AfterScheduleRecord | import("./types.ts").AtScheduleRecord | import("./types.ts").EveryScheduleRecord | import("./types.ts").DailyScheduleRecord | import("./types.ts").WeeklyScheduleRecord | import("./types.ts").CronScheduleRecord | import("./types.ts").RRuleScheduleRecord;
    createdAt?: string | undefined;
    updatedAt?: string | undefined;
    pendingDelivery?: {
        targetSessionId: SessionId;
    } | undefined;
}, {
    record: import("./types.ts").AfterScheduleRecord | import("./types.ts").AtScheduleRecord | import("./types.ts").EveryScheduleRecord | import("./types.ts").DailyScheduleRecord | import("./types.ts").WeeklyScheduleRecord | import("./types.ts").CronScheduleRecord | import("./types.ts").RRuleScheduleRecord;
    status: "active" | "paused" | "ended" | "inactive";
    sessionId?: unknown;
    target?: {
        kind: "session";
        sessionId: SessionId;
    } | {
        kind: "new_once";
        create: {
            cwd?: string;
            agentPreset?: string;
        };
        sessionId?: SessionId | undefined;
    } | {
        kind: "new";
        create: {
            cwd?: string;
            agentPreset?: string;
        };
    } | undefined;
    involvedSessionIds?: SessionId[] | undefined;
    createdAt?: string | undefined;
    updatedAt?: string | undefined;
    lastDelivery?: {
        scheduledAt: string;
        deliveredAt: string;
        messageId: MessageId;
        targetSessionId?: SessionId | undefined;
    } | undefined;
    deliveryHistory?: {
        records: {
            scheduledAt: string;
            deliveredAt: string;
            messageId: MessageId;
            targetSessionId?: SessionId | undefined;
            prompt?: string;
        }[];
        earlierRecordsUnavailable: boolean;
        earlierRecordsPruned: boolean;
    } | undefined;
    pendingDelivery?: {
        targetSessionId: SessionId;
    } | undefined;
}>>;
/** Persistent task value; a delivery receipt does not record model execution completion. */
export type ScheduleTask = z.infer<typeof scheduleTaskSchema>;
/** Authoritative Host tasks retain their domain, backend routing, and envelope; v2 also reads v1 without startup writes. */
export declare const scheduleDomain: {
    name: string;
    version: number;
    compatibleVersions: number[];
    tables: {
        tasks: import("@deepseek-ai/dsh-storage-domain").DomainTableSpec<ScheduleId, {
            deliveryHistory: {
                records: ScheduleDeliveryRecord[];
                earlierRecordsUnavailable: boolean;
                earlierRecordsPruned: boolean;
            };
            lastDelivery?: {
                scheduledAt: string;
                deliveredAt: string;
                messageId: MessageId;
                targetSessionId?: SessionId | undefined;
            } & {
                targetSessionId: SessionId;
            };
            target: ScheduleTarget;
            status: "active" | "paused" | "ended";
            involvedSessionIds: SessionId[];
            record: import("./types.ts").AfterScheduleRecord | import("./types.ts").AtScheduleRecord | import("./types.ts").EveryScheduleRecord | import("./types.ts").DailyScheduleRecord | import("./types.ts").WeeklyScheduleRecord | import("./types.ts").CronScheduleRecord | import("./types.ts").RRuleScheduleRecord;
            createdAt?: string | undefined;
            updatedAt?: string | undefined;
            pendingDelivery?: {
                targetSessionId: SessionId;
            } | undefined;
        }>;
    };
};
//# sourceMappingURL=storage.d.ts.map