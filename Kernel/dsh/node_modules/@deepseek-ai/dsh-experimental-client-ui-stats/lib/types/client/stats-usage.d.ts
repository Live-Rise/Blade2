/** Token accounting helpers shared by the Statistics table's step and turn levels. */
import type { StatsUsage } from './stats-contract.ts';
/**
 * Whether a value is a finite number.
 * @param value - candidate value.
 * @returns true when the value is a finite number.
 */
export declare function isNumber(value: unknown): value is number;
/**
 * Prompt length: input, cache read, and cache write do not overlap.
 * @param usage - usage reported by one assistant message.
 * @returns prompt token count, or null when nothing was reported.
 */
export declare function contextTokens(usage: StatsUsage | null): number | null;
/**
 * Output tokens reported by one usage object.
 * @param usage - usage reported by one assistant message.
 * @returns output token count, or null when unreported.
 */
export declare function outputTokens(usage: StatsUsage | null): number | null;
/**
 * Cache-read tokens reported by one usage object.
 * @param usage - usage reported by one assistant message.
 * @returns cache-read token count, or null when unreported.
 */
export declare function cacheReadTokens(usage: StatsUsage | null): number | null;
/**
 * Cache-write tokens reported by one usage object.
 * @param usage - usage reported by one assistant message.
 * @returns cache-write token count, or null when unreported.
 */
export declare function cacheWriteTokens(usage: StatsUsage | null): number | null;
/**
 * Share of the prompt served from cache; null when nothing was reported.
 * @param usage - usage reported by one assistant message.
 * @returns cache hit rate in [0, 1], or null when it cannot be derived.
 */
export declare function cacheHitRate(usage: StatsUsage | null): number | null;
//# sourceMappingURL=stats-usage.d.ts.map