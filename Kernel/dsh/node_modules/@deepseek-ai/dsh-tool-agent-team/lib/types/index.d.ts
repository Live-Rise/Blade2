/** Conditionally available task-board tools and guidance for Agent Teams. */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
/** Cordis plugin name. */
export declare const name = "tool-agent-team";
/** Services required by the Team tool plugin. */
export declare const inject: string[];
/** The Team tool plugin takes no configuration. */
export type Config = Record<never, never>;
/** Loader schema for the opt-in Team tool plugin. */
export declare const Config: z<Config>;
/**
 * Register Team tools and guidance selected by each caller's root Session mode.
 * @param ctx - context carrying Team, subagent, tool, and system prompt services.
 * @param config - the empty plugin configuration; removed keys fail at load.
 */
export declare function apply(ctx: Context, config?: Config): void;
//# sourceMappingURL=index.d.ts.map