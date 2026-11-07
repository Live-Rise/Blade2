/** Fixed-size recovery facts; positional attachment prices remain a rebuildable cache. */
import { z } from 'zod';
import type { TokenUsage } from '@deepseek-ai/dsh-llm';
import { SessionSeq } from '@deepseek-ai/dsh-session';
import type { SessionEvent } from '@deepseek-ai/dsh-session';
declare const measurementStateSchema: z.ZodObject<{
    headerSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
    stepStart: z.ZodNullable<z.ZodObject<{
        turn: z.ZodNumber;
        step: z.ZodNumber;
    }, z.core.$strict>>;
    anchor: z.ZodNullable<z.ZodObject<{
        headerSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
        assistantTokens: z.ZodNumber;
        usage: z.ZodNullable<z.ZodCustom<TokenUsage, TokenUsage>>;
    }, z.core.$strict>>;
    invalidSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
}, z.core.$strict>;
/** One header reference, one open step, and the latest successful call's usage facts. */
export type MeasurementState = z.infer<typeof measurementStateSchema>;
/**
 * Advance one measurement prefix without mutating its previous state.
 * @param state - measurement facts before the candidate event.
 * @param event - next committed journal event.
 * @returns updated measurement facts.
 * @throws the original measurement error for invalid boundaries or payloads.
 */
export declare function advanceMeasurement(state: MeasurementState, event: SessionEvent): MeasurementState;
/** Host-only measurement facts, independent of session and surface length. */
export declare const measurementProjectionDefinition: {
    key: "tokenMeasurement";
    stateVersion: number;
    stateSchema: z.ZodObject<{
        headerSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
        stepStart: z.ZodNullable<z.ZodObject<{
            turn: z.ZodNumber;
            step: z.ZodNumber;
        }, z.core.$strict>>;
        anchor: z.ZodNullable<z.ZodObject<{
            headerSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
            assistantTokens: z.ZodNumber;
            usage: z.ZodNullable<z.ZodCustom<TokenUsage, TokenUsage>>;
        }, z.core.$strict>>;
        invalidSeq: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
    }, z.core.$strict>;
    init: () => MeasurementState;
    apply: (state: NoInfer<{
        headerSeq: SessionSeq | null;
        stepStart: {
            turn: number;
            step: number;
        } | null;
        anchor: {
            headerSeq: SessionSeq | null;
            assistantTokens: number;
            usage: TokenUsage | null;
        } | null;
        invalidSeq: SessionSeq | null;
    }>, event: SessionEvent) => {
        headerSeq: SessionSeq | null;
        stepStart: {
            turn: number;
            step: number;
        } | null;
        anchor: {
            headerSeq: SessionSeq | null;
            assistantTokens: number;
            usage: TokenUsage | null;
        } | null;
        invalidSeq: SessionSeq | null;
    };
};
export {};
//# sourceMappingURL=measurement-projection.d.ts.map