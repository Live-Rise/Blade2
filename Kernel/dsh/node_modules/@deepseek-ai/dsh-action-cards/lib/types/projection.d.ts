/** Recover card proposals from logged `propose_action` calls and decisions from plugin records. */
import { z } from 'zod';
import { ToolCallId } from '@deepseek-ai/dsh-llm';
import { SessionLogOffset } from '@deepseek-ai/dsh-session';
import type { ProjectionDefinition } from '@deepseek-ai/dsh-session-projection';
/** Model-facing name of the card tool. */
export declare const PROPOSE_ACTION = "propose_action";
/** Kind-defined JSON object of a proposal or decision. */
export declare const fieldsSchema: z.ZodRecord<z.ZodString, z.ZodJSONSchema>;
/** Arguments of a `propose_action` call; blank titles and briefs create no card. */
export declare const argumentsSchema: z.ZodObject<{
    action: z.ZodString;
    title: z.ZodString;
    brief: z.ZodString;
    details: z.ZodRecord<z.ZodString, z.ZodJSONSchema>;
}, z.core.$strip>;
/** Recorded decision of one card. */
export declare const decisionSchema: z.ZodObject<{
    callId: z.ZodPipe<z.ZodString, z.ZodTransform<ToolCallId, string>>;
    action: z.ZodString;
    phase: z.ZodString;
    data: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodJSONSchema>>;
}, z.core.$strict>;
declare const stateSchema: z.ZodObject<{
    inheritedEventCount: z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionLogOffset, number>>;
    view: z.ZodObject<{
        proposals: z.ZodRecord<z.ZodString, z.ZodObject<{
            callId: z.ZodPipe<z.ZodString, z.ZodTransform<ToolCallId, string>>;
            turn: z.ZodNumber;
            action: z.ZodString;
            title: z.ZodString;
            brief: z.ZodString;
            details: z.ZodRecord<z.ZodString, z.ZodJSONSchema>;
        }, z.core.$strict>>;
        decisions: z.ZodRecord<z.ZodString, z.ZodObject<{
            callId: z.ZodPipe<z.ZodString, z.ZodTransform<ToolCallId, string>>;
            action: z.ZodString;
            phase: z.ZodString;
            data: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodJSONSchema>>;
        }, z.core.$strict>>;
    }, z.core.$strip>;
    calls: z.ZodRecord<z.ZodString, z.ZodObject<{
        details: z.ZodRecord<z.ZodString, z.ZodJSONSchema>;
        action: z.ZodString;
        title: z.ZodString;
        brief: z.ZodString;
        turn: z.ZodNumber;
    }, z.core.$strict>>;
    admittedTurn: z.ZodNullable<z.ZodNumber>;
    pendingStatus: z.ZodRecord<z.ZodString, z.ZodNumber>;
    queued: z.ZodNumber;
}, z.core.$strip>;
/** Fold state; `view` is also the Client wire value. */
export type ActionCardState = z.infer<typeof stateSchema>;
/** Type of {@link actionCardProjection}: the fold plus its Client view wire. */
export type ActionCardProjectionDefinition = Omit<ProjectionDefinition<'actionCards', ActionCardState>, 'wire'> & {
    readonly wire: NonNullable<ProjectionDefinition<'actionCards', ActionCardState>['wire']>;
};
/**
 * Full-Session fold. A successful `propose_action` result admits the call as a card, at most one per turn;
 * the first admitted call of a turn wins. A newer card of any kind replaces every unstarted earlier card.
 * Events inherited by a forked Session create no proposals or decisions.
 */
export declare const actionCardProjection: ActionCardProjectionDefinition;
export {};
//# sourceMappingURL=projection.d.ts.map