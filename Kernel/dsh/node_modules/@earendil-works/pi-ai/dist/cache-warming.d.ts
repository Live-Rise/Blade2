import type { Api, Model, SimpleStreamOptions } from "./types.ts";
/** Whether a refresh is sent ("warm") or warming ends until the next real request ("stop"). */
export type CacheWarmingAction = "warm" | "stop";
/** "streaming" while the agent run that sent the request is still active; "idle" after it settled. */
export type CacheWarmingPhase = "streaming" | "idle";
/** Inputs and outcome of one warm-or-stop decision. */
export interface CacheWarmingDecision {
    phase: CacheWarmingPhase;
    /** Price of this refresh: a cache read of the prompt plus one output token. */
    warmCost: number;
    /** Extra price of the next real request if the cache entry is lost. */
    missCost: number;
    /** Estimated chance that a real request arrives before the entry expires. */
    continuationProbability: number;
    /** `continuationProbability * missCost - warmCost`. */
    expectedSavings: number;
    /** False when the prompt size or the model's prices are unknown. */
    economicsAvailable: boolean;
    /** "warm" when `expectedSavings` is at least `CACHE_WARMING_MINIMUM_EXPECTED_SAVINGS`. */
    action: CacheWarmingAction;
}
/** Streaming warming never continues past this long after the real request that started it. */
export declare const CACHE_WARMING_MAX_AGE_MS: number;
/** Idle warming uses a shorter horizon because continuation estimates become less reliable with age. */
export declare const CACHE_WARMING_MAX_IDLE_AGE_MS: number;
/** A refresh is sent only when it is expected to save at least this many dollars. */
export declare const CACHE_WARMING_MINIMUM_EXPECTED_SAVINGS = 0.05;
/**
 * Chance that a real request arrives before the cache entry expires while the agent sits idle. Measured from our own
 * usage; per-session estimates were not better than this constant.
 */
export declare const CACHE_WARMING_IDLE_CONTINUATION_PROBABILITY = 0.15;
/** Refresh at 90% of the TTL while preserving at least ten seconds of margin. */
export declare function getCacheWarmingDelayMs(ttlMs: number): number | undefined;
/**
 * Lifetime of the prompt cache entry a request writes, from the model's `promptCache` tier for the retention the
 * request used. Undefined when the model has no lifetime for that tier or caching is off.
 */
export declare function getPromptCacheTtlMs(model: Model<Api>, options: SimpleStreamOptions | undefined): number | undefined;
/**
 * Whether replaying the request with a one-token output cap leaves its cache entry untouched. Anthropic's budget-based
 * thinking (Claude models without adaptive thinking) derives `budget_tokens` from `max_tokens`; the replay would get a
 * different budget, which Anthropic keys the message cache on, and the model could still think for thousands of tokens.
 */
export declare function isCacheWarmingReplayable(model: Model<Api>, options: SimpleStreamOptions | undefined): boolean;
/**
 * The options of a refresh: the request's own, capped at one output token, without retries, and without replacing the
 * conversation's provider thread state (the refresh repeats the last request).
 */
export declare function getCacheWarmingRequestOptions<T extends SimpleStreamOptions>(options: T, signal: AbortSignal): T;
/** Decide whether refreshing the cache entry of a `promptTokens`-token request pays for itself in `phase`. */
export declare function evaluateCacheWarming(model: Model<Api>, promptTokens: number, phase: CacheWarmingPhase): CacheWarmingDecision;
//# sourceMappingURL=cache-warming.d.ts.map