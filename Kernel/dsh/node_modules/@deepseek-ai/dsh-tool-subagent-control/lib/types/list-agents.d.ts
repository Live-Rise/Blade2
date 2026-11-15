/** Discover the caller's entire subagent tree; the control plugin owns registration. */
import type { Context } from '@deepseek-ai/cordis';
import type { ToolDefinition } from '@deepseek-ai/dsh-tools';
/**
 * Register the `list_agents` tool.
 * @param ctx - context carrying the tool registry, subagent service, and live Agent registry.
 * @returns the registered definition for scoped availability checks.
 */
export declare function registerListAgents(ctx: Context): ToolDefinition;
//# sourceMappingURL=list-agents.d.ts.map