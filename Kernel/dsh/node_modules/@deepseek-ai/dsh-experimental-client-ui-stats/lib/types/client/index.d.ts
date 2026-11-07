/**
 * Browser performance plugin contributing one entry to the conversation view
 * slot: per-turn and per-step context, timing, cache, and tool-cost columns.
 */
import type { Context } from '@deepseek-ai/cordis';
export type { StatsKey } from './locales.ts';
export type { StatsCall, StatsSnapshot, StatsStepMetrics, StatsTurnMetrics } from './stats-contract.ts';
/** Required services: the conversation slot and view registries, Session paging, and the locale service. */
export declare const inject: string[];
/**
 * Client plugin body: register the locale dictionaries, the event Definitions,
 * the view target, and the tab itself. Every registration rides the caller's
 * fiber effect, so unloading the plugin removes the tab with it.
 * @param ctx - client root context.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map