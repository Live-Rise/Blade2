/** Model-facing discovery of LLM routes available to child Agents. */
import type { Context } from '@deepseek-ai/cordis';
import type { ProjectionHandle } from '@deepseek-ai/dsh-session-projection';
import type { AllowedModelRoute } from './model-selection.ts';
/**
 * Register discovery of the calling Session’s authorized child models.
 * @param ctx - Context carrying the tool registry and LLM service.
 * @param policyHandle - retained reader for the Session’s authorized child routes.
 */
export declare function registerListSubagentModels(ctx: Context, policyHandle: ProjectionHandle<AllowedModelRoute[] | null>): void;
//# sourceMappingURL=list-models.d.ts.map