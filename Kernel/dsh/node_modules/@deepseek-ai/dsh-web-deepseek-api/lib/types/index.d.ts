/**
 * Register the DeepSeek Search and Fetch APIs in `ctx.web` as `deepseek-search-api`
 * and `deepseek-fetch-api`. Each operation is one direct API request without an
 * auxiliary model request. `web.searchProvider` and `web.fetchProvider` select them. Each capability's billing setting
 * chooses its credential. `auto` follows the conversation: a Session whose latest model request used the
 * `deepseek-account` route sends the account token to that route's origin and otherwise the API key; one on the
 * `deepseek-official` route sends the API key; any other operation sends the API key and, without a key, the signed-in
 * account's token. `account` sends only the account token and `api-key` only the API key. The API key is that of the
 * DeepSeek API-key route, sent to its origin, or `DEEPSEEK_API_KEY` sent to the official base without that route. The
 * account token goes to the account route's origin, or outside an account conversation without that route to the origin
 * of its default base URL. An operation that needs a mounted route whose settings do not resolve fails instead of
 * changing its endpoint or credential. A capability with its own `*BaseURL` or `*ApiKeyEnv` uses only that endpoint and
 * key instead, whatever its billing
 * ([rationale](../../../../.agents/notes/implemented/architecture/2026-10-29-web-provider-selection.md#credential-resolution)).
 * Both providers report themselves not ready when their next operation would have no credential.
 * @module @deepseek-ai/dsh-web-deepseek-api
 */
import { type Context, type Volatile } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
export { DEEPSEEK_FETCH_API_PROVIDER_ID, DeepSeekFetchApiProvider } from './fetch.ts';
export type { DeepSeekFetchApiOptions } from './fetch.ts';
export { DEEPSEEK_DEFAULT_COUNT, DEEPSEEK_SEARCH_API_PROVIDER_ID, DEEPSEEK_SEARCH_REQUESTS_PER_SECOND, DEEPSEEK_WEB_API_BASE_URL, DeepSeekSearchApiProvider, mapSearchResponse, } from './search.ts';
export type { DeepSeekSearchApiOptions } from './search.ts';
/** Cordis plugin name used by loader diagnostics. */
export declare const name = "web-deepseek-api";
/** The web seam both providers register into. */
export declare const inject: string[];
/**
 * Settings read by the next search or fetch without re-registering either provider. A non-empty `*BaseURL` or
 * `*ApiKeyEnv` gives that capability its own endpoint and key; with neither, its `*Billing` chooses its DeepSeek
 * credential. Stored fields of earlier settings (`baseURL`, `apiKeyEnv`, `apiKey`) are not schema fields: they load
 * without error and have no effect.
 */
export interface Config {
    /** Search API results per search, from 1 through 50; a smaller request bound wins. Defaults to 10. */
    count: Volatile<number>;
    /**
     * Most Search requests this plugin instance sends in any one-second window; later searches wait,
     * and the wait counts against the caller's timeout. Other processes and clients on the same account
     * share the account limit without coordination. Defaults to 5.
     */
    searchRequestsPerSecond: Volatile<number>;
    /** Send the triggering user question, bounded to 128 characters, as `ori_question`. Defaults to true. */
    sendOriginalQuestion: Volatile<boolean>;
    /**
     * Credential Search sends without {@link searchBaseURL} and {@link searchApiKeyEnv}. `auto` uses the conversation's
     * DeepSeek account or API key while it chats on a DeepSeek model route, and otherwise the API key, then the signed-in
     * account; `account` uses only the signed-in account; `api-key` uses only the API key. Defaults to `auto`.
     */
    searchBilling: Volatile<'auto' | 'account' | 'api-key'>;
    /** Credential Fetch sends without {@link fetchBaseURL} and {@link fetchApiKeyEnv}, chosen as {@link searchBilling} is. */
    fetchBilling: Volatile<'auto' | 'account' | 'api-key'>;
    /**
     * Own Search API base including its path prefix; `/search` is appended. With only {@link searchApiKeyEnv} set, the
     * official base applies.
     */
    searchBaseURL: Volatile<string | undefined>;
    /** Own Search API credential reference. With only {@link searchBaseURL} set, `DEEPSEEK_API_KEY` applies. */
    searchApiKeyEnv: Volatile<string | undefined>;
    /**
     * Own Fetch API base including its path prefix; `/fetch` is appended. With only {@link fetchApiKeyEnv} set, the
     * official base applies.
     */
    fetchBaseURL: Volatile<string | undefined>;
    /** Own Fetch API credential reference. With only {@link fetchBaseURL} set, `DEEPSEEK_API_KEY` applies. */
    fetchApiKeyEnv: Volatile<string | undefined>;
}
export declare const Config: z<Schemastery.ObjectS<NoInfer<{
    count: z<number, number, "volatile-defined">;
    searchRequestsPerSecond: z<number, number, "volatile-defined">;
    sendOriginalQuestion: z<boolean, boolean, "volatile-defined">;
    searchBilling: z<"account" | "api-key" | "auto", "account" | "api-key" | "auto", "volatile-defined">;
    fetchBilling: z<"account" | "api-key" | "auto", "account" | "api-key" | "auto", "volatile-defined">;
    searchBaseURL: z<string, string, "volatile">;
    searchApiKeyEnv: z<string, string, "volatile">;
    fetchBaseURL: z<string, string, "volatile">;
    fetchApiKeyEnv: z<string, string, "volatile">;
}>>, Schemastery.ObjectT<NoInfer<{
    count: z<number, number, "volatile-defined">;
    searchRequestsPerSecond: z<number, number, "volatile-defined">;
    sendOriginalQuestion: z<boolean, boolean, "volatile-defined">;
    searchBilling: z<"account" | "api-key" | "auto", "account" | "api-key" | "auto", "volatile-defined">;
    fetchBilling: z<"account" | "api-key" | "auto", "account" | "api-key" | "auto", "volatile-defined">;
    searchBaseURL: z<string, string, "volatile">;
    searchApiKeyEnv: z<string, string, "volatile">;
    fetchBaseURL: z<string, string, "volatile">;
    fetchApiKeyEnv: z<string, string, "volatile">;
}>>, "plain">;
/**
 * Register the Search API and Fetch API providers over one live settings section. Each operation reads one settings
 * snapshot and the route of the initiating Session's latest model request; each `ready()` call reads one settings
 * snapshot and the route of the model request being prepared. Both resolve the same ordered credentials.
 * @param ctx - plugin context owning both registrations.
 * @param config - live settings, read once per operation.
 */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map