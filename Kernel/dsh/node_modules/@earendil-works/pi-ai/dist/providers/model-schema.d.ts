import { type Static } from "@earendil-works/pi-schema";
export declare const THINKING_LEVELS: readonly ["minimal", "low", "medium", "high", "xhigh", "max"];
export declare const MODEL_THINKING_LEVELS: readonly ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
export declare const MODEL_INPUT_MODALITIES: readonly ["text", "image"];
export declare const CACHE_RETENTIONS: readonly ["none", "short", "long"];
export declare const MODEL_PROMPT_CACHE_RETENTIONS: readonly ["short", "long"];
export declare const ThinkingLevelSchema: import("@earendil-works/pi-schema").Schema<"high" | "low" | "max" | "medium" | "minimal" | "xhigh">;
export declare const ModelThinkingLevelSchema: import("@earendil-works/pi-schema").Schema<"high" | "low" | "max" | "medium" | "minimal" | "off" | "xhigh">;
export declare const ModelInputModalitySchema: import("@earendil-works/pi-schema").Schema<"image" | "text">;
export declare const CacheRetentionSchema: import("@earendil-works/pi-schema").Schema<"long" | "none" | "short">;
export declare const ThinkingLevelMapSchema: import("@earendil-works/pi-schema").Schema<{
    off?: string | null | undefined;
    minimal?: string | null | undefined;
    low?: string | null | undefined;
    medium?: string | null | undefined;
    high?: string | null | undefined;
    xhigh?: string | null | undefined;
    max?: string | null | undefined;
}>;
export declare const ModelPromptCacheSchema: import("@earendil-works/pi-schema").Schema<{
    short?: number | undefined;
    long?: number | undefined;
}>;
export declare const ModelCostRatesSchema: import("@earendil-works/pi-schema").Schema<{
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
}>;
export declare const ModelCostTierSchema: import("@earendil-works/pi-schema").Schema<{
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
    inputTokensAbove: number;
}>;
export declare const ModelCostSchema: import("@earendil-works/pi-schema").Schema<{
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
    tiers?: {
        input: number;
        output: number;
        cacheRead: number;
        cacheWrite: number;
        inputTokensAbove: number;
    }[] | undefined;
}>;
export declare const ModelImageResizeOptionsSchema: import("@earendil-works/pi-schema").Schema<{
    maxWidth?: number | undefined;
    maxHeight?: number | undefined;
    maxBytes?: number | undefined;
    jpegQuality?: number | undefined;
}>;
export declare const ModelImageInputLimitsSchema: import("@earendil-works/pi-schema").Schema<{
    resize?: {
        maxWidth?: number | undefined;
        maxHeight?: number | undefined;
        maxBytes?: number | undefined;
        jpegQuality?: number | undefined;
    } | undefined;
    maxPerMessage?: number | undefined;
    maxPerRequest?: number | undefined;
}>;
export declare const ModelInputLimitsSchema: import("@earendil-works/pi-schema").Schema<{
    maxRequestBytes?: number | undefined;
    images?: {
        resize?: {
            maxWidth?: number | undefined;
            maxHeight?: number | undefined;
            maxBytes?: number | undefined;
            jpegQuality?: number | undefined;
        } | undefined;
        maxPerMessage?: number | undefined;
        maxPerRequest?: number | undefined;
    } | undefined;
}>;
export type ThinkingLevel = Static<typeof ThinkingLevelSchema>;
export type ModelThinkingLevel = Static<typeof ModelThinkingLevelSchema>;
export type ThinkingLevelMap = Static<typeof ThinkingLevelMapSchema>;
export type ModelInputModality = Static<typeof ModelInputModalitySchema>;
export type CacheRetention = Static<typeof CacheRetentionSchema>;
export type ModelPromptCache = Static<typeof ModelPromptCacheSchema>;
export type ModelCostRates = Static<typeof ModelCostRatesSchema>;
export type ModelCostTier = Static<typeof ModelCostTierSchema>;
export type ModelCost = Static<typeof ModelCostSchema>;
export type ModelImageResizeOptions = Static<typeof ModelImageResizeOptionsSchema>;
export type ModelImageInputLimits = Static<typeof ModelImageInputLimitsSchema>;
export type ModelInputLimits = Static<typeof ModelInputLimitsSchema>;
//# sourceMappingURL=model-schema.d.ts.map