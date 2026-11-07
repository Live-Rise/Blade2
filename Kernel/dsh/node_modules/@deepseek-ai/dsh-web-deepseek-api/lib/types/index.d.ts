/**
 * Register the DeepSeek Search and Fetch APIs in `ctx.web` as `deepseek-search-api`
 * and `deepseek-fetch-api`. Each operation is one direct API request without an
 * auxiliary model request; both providers read this section's credential and API
 * base. `web.searchProvider` and `web.fetchProvider` select them. A Session on the
 * DeepSeek account route authenticates with the account token when the account
 * service allows the endpoint; every other request uses the configured key. Both
 * providers report themselves not ready for a model request whose operation would
 * have no credential.
 * @module @deepseek-ai/dsh-web-deepseek-api
 */
import type { Context, Volatile } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
export { DEEPSEEK_FETCH_API_PROVIDER_ID, DeepSeekFetchApiProvider } from './fetch.ts';
export type { DeepSeekFetchApiOptions } from './fetch.ts';
export { DEEPSEEK_DEFAULT_COUNT, DEEPSEEK_SEARCH_API_PROVIDER_ID, DEEPSEEK_WEB_API_BASE_URL, DeepSeekSearchApiProvider, mapSearchResponse, } from './search.ts';
export type { DeepSeekSearchApiOptions } from './search.ts';
/** Cordis plugin name used by loader diagnostics. */
export declare const name = "web-deepseek-api";
/** The web seam both providers register into. */
export declare const inject: string[];
/** Settings read by the next search or fetch without re-registering either provider. */
export interface Config {
    /** Literal DeepSeek API key; prefer {@link apiKeyEnv} so no secret enters configuration files. */
    apiKey: Volatile<string | undefined>;
    /** Credential reference resolved for each operation; defaults to `DEEPSEEK_API_KEY`. */
    apiKeyEnv: Volatile<string>;
    /** API base including its path prefix; `/search` and `/fetch` are appended. */
    baseURL: Volatile<string>;
    /** Search API results per search, from 1 through 50; a smaller request bound wins. Defaults to 10. */
    count: Volatile<number>;
    /** Send the triggering user question, bounded to 128 characters, as `ori_question`. Defaults to true. */
    sendOriginalQuestion: Volatile<boolean>;
}
export declare const Config: z<Schemastery.ObjectS<NoInfer<{
    apiKey: z<string, string, "volatile">;
    apiKeyEnv: z<string, string, "volatile-defined">;
    baseURL: z<string, string, "volatile-defined">;
    count: z<number, number, "volatile-defined">;
    sendOriginalQuestion: z<boolean, boolean, "volatile-defined">;
}>>, Schemastery.ObjectT<NoInfer<{
    apiKey: z<string, string, "volatile">;
    apiKeyEnv: z<string, string, "volatile-defined">;
    baseURL: z<string, string, "volatile-defined">;
    count: z<number, number, "volatile-defined">;
    sendOriginalQuestion: z<boolean, boolean, "volatile-defined">;
}>>, "plain">;
/**
 * Register the Search API and Fetch API providers over one live settings section. Each provider's
 * `ready()` answers whether an operation for a model request on the given route would have a
 * credential to send.
 * @param ctx - plugin context owning both registrations.
 * @param config - live settings, read once per operation.
 */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map