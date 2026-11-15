import type { Tool } from "../types.ts";
/** Returns true when a provider's strict mode rejects this schema keyword with this value. */
export type UnsupportedStrictSchemaKeywordCheck = (key: string, value: unknown) => boolean;
/** Convert a tool schema to the strict subset expected by provider constrained sampling. */
export declare function makeStrictJsonSchema(schema: Tool["parameters"], isUnsupportedKeyword?: UnsupportedStrictSchemaKeywordCheck): Record<string, unknown>;
/** Returns true for a tool that asks for strict JSON-schema sampling only when possible (`strict: "prefer"`). */
export declare function prefersJsonSchemaStrictSampling(tool: Tool): boolean;
export declare function getJsonSchemaToolParameters(tool: Tool, strict: boolean | undefined): Tool["parameters"];
export interface GrammarConstrainedSampling {
    format: "lark" | "regex";
    definition: string;
    inputProperty: string;
}
export interface GrammarToolInputJsonBuffer {
    input: string;
    started: boolean;
    closed: boolean;
}
export declare function getGrammarToolInput(toolName: string, arguments_: Record<string, unknown>, inputProperty: string): string;
/**
 * Append `inputDelta` to a streaming grammar tool input and return the matching piece of its JSON arguments text, or
 * `undefined` for an empty delta. Costs time linear in the delta, never in the input so far.
 */
export declare function appendGrammarToolInputJsonDelta(buffer: GrammarToolInputJsonBuffer, inputProperty: string, inputDelta: string): string | undefined;
/**
 * Close a streaming grammar tool input and return the rest of its JSON arguments text. `finalInput` is the complete
 * input when the provider sends one; it must continue the input streamed so far. Closing again with the same input
 * returns `undefined`.
 */
export declare function closeGrammarToolInputJson(buffer: GrammarToolInputJsonBuffer, inputProperty: string, finalInput: string | undefined): string | undefined;
/**
 * Decide whether a JSON-schema tool is sent in strict mode. `isUnsupportedKeyword` lets a provider
 * reject extra keywords its strict mode does not accept, so "prefer" tools fall back to non-strict.
 */
export declare function resolveJsonSchemaStrictSampling(tool: Tool, supportsStrictMode: boolean, isUnsupportedKeyword?: UnsupportedStrictSchemaKeywordCheck): boolean | undefined;
export declare function resolveGrammarConstrainedSampling(tool: Tool, supportsOpenAIGrammarTools: boolean): GrammarConstrainedSampling | undefined;
export declare function createGrammarToolInputProperties(tools: Tool[] | undefined, supportsOpenAIGrammarTools: boolean): ReadonlyMap<string, string>;
//# sourceMappingURL=constrained-sampling.d.ts.map