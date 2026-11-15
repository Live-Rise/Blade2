import type { ProviderStreams } from "../types.ts";
/**
 * Overrides the dynamically imported bedrock implementation. Used by the Bun
 * binary build, where the variable-specifier import cannot be bundled; the
 * build registers a statically imported module instead.
 */
export declare function setBedrockProviderModule(module: ProviderStreams): void;
/**
 * Overrides how the bedrock implementation is imported, loaded on the first
 * bedrock request. Used by Node bundles that split at dynamic imports (see
 * `@earendil-works/pi-ai/lazy-bedrock-provider`): the loader's literal
 * `import()` lets the bundler keep the implementation in a lazy chunk.
 */
export declare function setBedrockProviderLoader(load: () => Promise<ProviderStreams>): void;
export declare const bedrockConverseStreamApi: () => ProviderStreams;
//# sourceMappingURL=bedrock-converse-stream.lazy.d.ts.map