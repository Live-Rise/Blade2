/**
 * Agent-scoped consumers of the shared Host Schedule management service.
 * @module @deepseek-ai/dsh-schedule
 */
import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
/**
 * Register all four Schedule tools in one exact agent scope.
 * @param rootCtx - Host context owning the shared Schedule service.
 * @param toolCtx - Exact agent-scoped context receiving the definitions.
 * @param agent - Exact live owner whose session the tools mutate.
 * @returns Idempotent aggregate disposer for the four registrations.
 */
export declare function registerScheduleTools(rootCtx: Context, toolCtx: Context, agent: Agent): () => void;
//# sourceMappingURL=tools.d.ts.map