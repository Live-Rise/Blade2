/**
 * DeepSeek Fetch API provider: page Markdown through one direct request, without
 * a model request.
 * @module @deepseek-ai/dsh-web-deepseek-api/fetch
 */
import type { WebFetchProvider, WebFetchRequest, WebFetchResult } from '@deepseek-ai/dsh-web';
import { type DeepSeekWebAuth } from './transport.ts';
/** Stable id the Fetch API provider registers under in the fetch registry. */
export declare const DEEPSEEK_FETCH_API_PROVIDER_ID = "deepseek-fetch-api";
/** Authentication and endpoint settings snapshotted for one fetch. */
export interface DeepSeekFetchApiOptions extends DeepSeekWebAuth {
    /** API base including its path prefix; `/fetch` is appended. */
    baseURL: string;
}
/**
 * Fetch page Markdown through the DeepSeek Fetch API. The API reports neither the
 * target's status nor a final URL, and it fails when the target yields no usable
 * content, so a success is the requested page retrieved with HTTP 200.
 */
export declare class DeepSeekFetchApiProvider implements WebFetchProvider {
    private readonly resolveOptions;
    readonly id = "deepseek-fetch-api";
    /** @param resolveOptions - the options for the NEXT fetch, read once at each fetch's entry. */
    constructor(resolveOptions: () => DeepSeekFetchApiOptions);
    available(): boolean;
    fetch(request: WebFetchRequest, signal?: AbortSignal): Promise<WebFetchResult>;
}
//# sourceMappingURL=fetch.d.ts.map