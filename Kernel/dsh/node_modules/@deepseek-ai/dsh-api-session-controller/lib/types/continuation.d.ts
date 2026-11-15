import type { SessionSeq } from '@deepseek-ai/dsh-session/types';
import { z } from 'zod';
declare const stateSchema: z.ZodObject<{
    admitted: z.ZodBoolean;
    failedAttempt: z.ZodBoolean;
    ending: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
}, z.core.$strip>;
/** Recoverable manual-continuation facts over admitted user input and empty retry messages. */
export type ContinuationState = z.infer<typeof stateSchema>;
/** Eligibility requires admitted human work and a failed settlement in the last request step. */
export declare const continuationProjectionDefinition: {
    key: "conversationContinuation";
    stateVersion: number;
    stateSchema: z.ZodObject<{
        admitted: z.ZodBoolean;
        failedAttempt: z.ZodBoolean;
        ending: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
    }, z.core.$strip>;
    init: () => ContinuationState;
    apply: (state: NoInfer<{
        admitted: boolean;
        failedAttempt: boolean;
        ending: SessionSeq | null;
    }>, event: import("@deepseek-ai/dsh-session").SessionEvent) => {
        admitted: boolean;
        failedAttempt: boolean;
        ending: SessionSeq | null;
    };
    wire: {
        viewSchema: z.ZodNullable<z.ZodPipe<z.ZodNumber, z.ZodTransform<SessionSeq, number>>>;
        view: (state: ContinuationState) => SessionSeq | null;
    };
};
export {};
//# sourceMappingURL=continuation.d.ts.map