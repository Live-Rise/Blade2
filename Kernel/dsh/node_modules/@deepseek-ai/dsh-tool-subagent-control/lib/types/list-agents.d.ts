/**
 * The globally named `list_agents` tool: a thin model-facing adapter over
 * the continuable projection of `ctx.subagents.listChildren()` and, for the
 * `descendants` scope, `ctx.subagents.listDescendants()`. The control plugin owns
 * registration and disposal alongside messaging and interruption.
 */
import type { Context } from '@deepseek-ai/cordis';
/**
 * Register the `list_agents` tool.
 * @param ctx - context carrying the tool registry, subagent service, and live Agent registry.
 */
export declare function registerListAgents(ctx: Context): void;
//# sourceMappingURL=list-agents.d.ts.map