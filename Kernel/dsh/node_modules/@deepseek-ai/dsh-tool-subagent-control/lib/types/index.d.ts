/**
 * Shared messaging, inbox waiting, interruption, and child discovery tools.
 * They perform no lifecycle routing of their own —
 * residency, cold resume, and interrupt authorization belong to the subagent
 * service — and they live apart from the provider-bound
 * `@deepseek-ai/dsh-tool-subagent` instances so multiple delegation tools share
 * one control API.
 * @module @deepseek-ai/dsh-tool-subagent-control
 */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
export declare const name = "tool-subagent-control";
export declare const inject: string[];
/** Optional subagent control tools. */
export interface Config {
    /** Register `interrupt_agent`. Defaults to true; false leaves the service API available. */
    interrupt?: boolean;
}
/** Loader schema for optional subagent control tools. */
export declare const Config: z<Config>;
/**
 * Register the shared subagent control and discovery tools.
 * @param ctx - context carrying tools, subagents, live Agents, and prompt assembly.
 * @param config - optional tool registration settings.
 */
export declare function apply(ctx: Context, config?: Config): void;
//# sourceMappingURL=index.d.ts.map