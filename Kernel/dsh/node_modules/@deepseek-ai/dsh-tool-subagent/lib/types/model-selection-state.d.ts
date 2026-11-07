/** Durable per-session state for the user-controlled model-selection opt-in. */
import { z as zod } from 'zod';
import type { Session } from '@deepseek-ai/dsh-session';
import type { ProjectionHandle } from '@deepseek-ai/dsh-session-projection';
import { type AllowedModelRoute } from './model-selection.ts';
declare module '@deepseek-ai/dsh-session/types' {
    interface SessionEventMap {
        /**
         * Records this Session’s authorized child provider/model routes. Appended
         * before the first model request; absence restricts explicit selection
         * to the current parent route. Log-only: it carries
         * no `surfaceOp` and never enters model history.
         */
        'subagent/model-selection-policy': {
            /** Exact routes this Session may select explicitly for a child. */
            allowedModels: AllowedModelRoute[];
        };
    }
}
/**
 * Host-only projection of the durable model-selection policy.
 * Each independently mounted consumer registers this shared definition; the registry reference-counts it.
 */
export declare const subagentModelSelectionProjectionDefinition: {
    key: "subagentModelSelectionPolicy";
    stateVersion: number;
    stateSchema: zod.ZodType<AllowedModelRoute[] | null, unknown, zod.core.$ZodTypeInternals<AllowedModelRoute[] | null, unknown>>;
    init: () => null;
    apply: (policy: NoInfer<AllowedModelRoute[] | null>, event: import("@deepseek-ai/dsh-session").SessionEvent) => AllowedModelRoute[] | null;
};
/**
 * Read the exact route list authorized for a Session.
 * @param subagentModelSelectionProjectionHandle - registered policy state reader.
 * @param session - session whose durable decision is read.
 * @returns a detached route list, or undefined to restrict selection to the parent route.
 */
export declare function subagentModelSelectionPolicy(subagentModelSelectionProjectionHandle: Pick<ProjectionHandle<AllowedModelRoute[] | null>, 'stateOf'>, session: Session): AllowedModelRoute[] | undefined;
//# sourceMappingURL=model-selection-state.d.ts.map