/**
 * Authentication and redirect-rejecting transport shared by the Search and
 * Fetch API providers in this package
 * ([rationale](../../../../.agents/notes/implemented/feature/2026-10-15-deepseek-search-fetch-api.md)).
 * @module @deepseek-ai/dsh-web-deepseek-api/transport
 */
import type { CredentialRef } from '@deepseek-ai/dsh-credentials';
import { WebError } from '@deepseek-ai/dsh-web';
/**
 * API key of one {@link DeepSeekWebAuthSource}: a literal value, where an empty string means no key, or a credential
 * reference read once per use without retention.
 */
export type DeepSeekWebApiKey = string | {
    /** Credential reference the key is stored under. */
    ref: CredentialRef;
    /** Report key presence without reading or validating its value; store failures reject. */
    configured: () => Promise<boolean>;
    /** Read and validate the key; undefined or empty means no key, and store or format failures reject. */
    resolve: () => Promise<string | undefined>;
};
/**
 * One credential an operation can send, bound to the API base it is sent to. The operation appends its path, such as
 * `/search`, to `baseURL`.
 */
export type DeepSeekWebAuthSource = 
/** The account token released for the dispatch endpoint, sent as `x-dsh-auth-token`. */
{
    kind: 'account';
    /** API base the token is sent to, including its path prefix. */
    baseURL: string;
    /** Read the token released for a dispatch endpoint; undefined or empty means no token, and read failures reject. */
    resolveToken: (endpoint: string) => Promise<string | undefined>;
    /** Invalidate the exact token rejected by HTTP 401; failures must not replace the request error. */
    rejectToken: (token: string) => Promise<void>;
}
/** An API key, sent as `Authorization: Bearer`. */
 | {
    kind: 'api-key';
    /** API base the key is sent to, including its path prefix. */
    baseURL: string;
    /** The key to send. */
    key: DeepSeekWebApiKey;
}
/** A credential whose settings do not resolve; reaching it fails the operation with `WEB_PROVIDER_MISCONFIGURED`. */
 | {
    kind: 'misconfigured';
    /** Model-facing diagnostic naming the settings and their recoveries. */
    message: string;
};
/**
 * Credentials of one operation in the order the caller chose. The operation reads them in order and sends one request
 * with the first that is present; a later source is not read once an earlier one is present, fails to read, or is
 * misconfigured. Request failures never move to another source.
 */
export interface DeepSeekWebAuth {
    /** Candidate credentials, first preferred. */
    sources: readonly DeepSeekWebAuthSource[];
    /** Model-facing `WEB_PROVIDER_CREDENTIAL_MISSING` diagnostic; omitted uses a generic missing-credential message. */
    missingCredentialMessage?: string;
}
/** User-owned endpoint settings named by dispatch failures. */
export interface DeepSeekWebGuidance {
    /** Capability whose endpoint failed. */
    subject: 'Search' | 'Fetch';
    /** Plugin configuration namespace. */
    plugin: string;
    /** Whether the endpoint is the capability's own base URL rather than one derived from a model route. */
    custom: boolean;
    /** Configuration field of the capability's own base URL. */
    baseURLField: string;
    /** Configuration field of the capability's own credential reference. */
    apiKeyEnvField: string;
}
/** One fully resolved web API request. */
interface DeepSeekWebRequest<T> {
    /** Operation path appended to the base of the source sent, such as `/search`. */
    path: string;
    /** JSON request body. */
    body: object;
    /** Protocol headers without credentials. */
    headers?: Readonly<Record<string, string>>;
    /** Additional API-key headers; not applied to account tokens. */
    apiKeyHeaders?: (key: string) => Readonly<Record<string, string>>;
    /** Validate and project the response. */
    decode: (payload: unknown) => T;
    /** Endpoint recovery settings. */
    guidance: DeepSeekWebGuidance;
    /** Record model input before dispatch; a throw prevents dispatch. */
    recordRequest?: (endpoint: string) => void;
    /** Wait for permission to send after credentials resolve; a rejection prevents dispatch. */
    waitToSend?: (signal?: AbortSignal) => Promise<void>;
}
/**
 * Check synchronously whether an operation has a source to try, without reading credentials.
 * @param auth - Sources for the operation.
 * @returns Whether any source is an account, a credential reference, a non-empty literal key, or a misconfigured source
 *   the operation reports.
 */
export declare function hasDeepSeekWebCredential(auth: DeepSeekWebAuth): boolean;
/**
 * Check whether an operation would find a credential, reading the sources in the order {@link dispatchDeepSeekWeb}
 * does. An account source reads its token for the operation's endpoint; a referenced key is only checked for presence,
 * so a malformed or wrong key counts as ready. A misconfigured source or a failed read answers false without reading
 * later sources, because the operation fails there too. Nothing is sent and nothing is retained.
 * @param auth - Sources for the operation.
 * @param path - Operation path appended to each source's base.
 * @returns Whether a source before any misconfigured or unreadable one is present.
 */
export declare function isDeepSeekWebAuthReady(auth: DeepSeekWebAuth, path: '/search' | '/fetch'): Promise<boolean>;
/** Narrow a remote JSON object.
 * @param value - Untrusted JSON.
 * @returns Whether readable object fields exist.
 */
export declare function isRecord(value: unknown): value is Record<string, unknown>;
/** Report malformed upstream JSON without embedding response content.
 * @returns The stable provider error.
 */
export declare function invalidResponse(): WebError;
/** Dispatch one operation using a single settings and credential snapshot: one request, with the first present source.
 * @param options - Ordered authentication sources for this operation.
 * @param request - Operation path, JSON payload, and response decoder.
 * @param signal - Caller cancellation signal.
 * @returns The validated provider result.
 */
export declare function dispatchDeepSeekWeb<T>(options: DeepSeekWebAuth, request: DeepSeekWebRequest<T>, signal?: AbortSignal): Promise<T>;
/**
 * Build the provider's stable cancellation error while retaining the caller's reason.
 * @param signal - the aborted caller signal; its reason becomes the cause.
 * @param fallback - the cause when the signal has not aborted, such as a fetch `AbortError`.
 * @returns a `WEB_ABORTED` error.
 */
export declare function searchAborted(signal?: AbortSignal, fallback?: unknown): WebError;
export {};
//# sourceMappingURL=transport.d.ts.map