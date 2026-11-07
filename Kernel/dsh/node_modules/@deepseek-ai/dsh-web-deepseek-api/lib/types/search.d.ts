/**
 * DeepSeek Search API provider: one direct request per search, without an
 * auxiliary model request.
 * @module @deepseek-ai/dsh-web-deepseek-api/search
 */
import type { WebSearchProvider, WebSearchRequest, WebSearchResult } from '@deepseek-ai/dsh-web';
import { type DeepSeekWebAuth, type DeepSeekWebGuidance } from './transport.ts';
/** Stable id the Search API provider registers under in the search registry. */
export declare const DEEPSEEK_SEARCH_API_PROVIDER_ID = "deepseek-search-api";
/** Default Search and Fetch API base, including its `/v1/tools` prefix; `/search` and `/fetch` are appended. */
export declare const DEEPSEEK_WEB_API_BASE_URL = "https://api.deepseek.com/v1/tools";
/** Default Search API result count, the service's own default. */
export declare const DEEPSEEK_DEFAULT_COUNT = 10;
/** Settings named by endpoint failures of either API in this package. */
export declare const DEEPSEEK_WEB_API_GUIDANCE: Omit<DeepSeekWebGuidance, 'subject'>;
/** Authentication, endpoint, and result settings snapshotted for one search. */
export interface DeepSeekSearchApiOptions extends DeepSeekWebAuth {
    /** API base including its path prefix; `/search` is appended. */
    baseURL: string;
    /** Results requested per search, from 1 through 50; a smaller request bound wins. */
    count: number;
    /** Read the user question that triggered the search; undefined or blank sends none. */
    originalQuestion?: () => string | undefined;
}
/**
 * Validate a Search API response and project its sources into the web service.
 * @param response - untrusted JSON from the Search endpoint.
 * @returns sources with provider order, the display `snippet`, the model-facing `summary`, and nullable
 *   publication dates preserved.
 * @throws {@link WebError} when the response does not contain valid search results.
 */
export declare function mapSearchResponse(response: unknown): WebSearchResult;
/**
 * Search through the DeepSeek Search API. HTTP redirects fail as `WEB_PROVIDER_ERROR`;
 * failures after dispatch name the endpoint and tell the model how the user can configure it.
 */
export declare class DeepSeekSearchApiProvider implements WebSearchProvider {
    private readonly resolveOptions;
    readonly id = "deepseek-search-api";
    /**
     * @param resolveOptions - the options for the NEXT search, read once at each
     * search's entry so one search never mixes two settings sections.
     */
    constructor(resolveOptions: () => DeepSeekSearchApiOptions);
    available(): boolean;
    search(request: WebSearchRequest, signal?: AbortSignal): Promise<WebSearchResult>;
}
//# sourceMappingURL=search.d.ts.map