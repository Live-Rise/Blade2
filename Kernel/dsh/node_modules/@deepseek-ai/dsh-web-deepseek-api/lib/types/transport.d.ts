/**
 * Authentication and redirect-rejecting transport shared by the Search and
 * Fetch API providers in this package. Credential resolution and its readiness
 * check, the dispatch call, and the cancellation helpers deliberately repeat
 * `dsh-web-search-deepseek`, so neither plugin depends on the other; the
 * `jscpd:ignore` regions mark those repeats
 * ([rationale](../../../../.agents/notes/implemented/feature/2026-10-15-deepseek-search-fetch-api.md)).
 * @module @deepseek-ai/dsh-web-deepseek-api/transport
 */
import type { Context } from '@deepseek-ai/cordis';
import { type CredentialRef } from '@deepseek-ai/dsh-credentials';
import { WebError } from '@deepseek-ai/dsh-web';
/** Authentication and endpoint values resolved once for an operation. */
export interface DeepSeekWebAuth {
    /** Resolve an allowed account token; undefined selects API-key authentication. */
    resolveAccountToken?: (endpoint: string) => Promise<string | undefined>;
    /** Literal API key; takes precedence over the credential resolver. */
    apiKey?: string;
    /** Resolve the current API key without retaining it. */
    resolveApiKey?: () => Promise<string | undefined>;
    /** Credential reference named by missing-key diagnostics. */
    apiKeyEnv?: CredentialRef;
}
/** User-owned endpoint settings named by dispatch failures. */
export interface DeepSeekWebGuidance {
    /** Capability whose endpoint failed. */
    subject: 'Search' | 'Fetch';
    /** Plugin configuration namespace. */
    plugin: string;
    /** Endpoint configuration field. */
    field: string;
    /** Local settings label. */
    label: string;
    /** Built-in settings page title. */
    page: string;
}
/** One fully resolved web API request. */
interface DeepSeekWebRequest<T> {
    /** Absolute dispatch URL. */
    endpoint: string;
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
}
/**
 * Resolve credential sources for one operation without retaining secret values.
 * @param ctx - Provider context with request routing and credential services.
 * @param config - Literal key and credential reference from the settings snapshot.
 * @returns Authentication sources for one operation.
 */
export declare function deepSeekWebAuth(ctx: Context, config: {
    apiKey: string | undefined;
    apiKeyEnv: string;
}): DeepSeekWebAuth;
/**
 * Whether an operation for a model request on `route` has a credential, checked in the order the
 * operation resolves them: the account token first on the account route, then the API key. A failed
 * read answers false, because the operation fails on that read instead of trying the next credential.
 * Only presence is checked: the key's value is not read and the endpoint is not contacted, so a wrong
 * key still counts as configured.
 * @param ctx - Provider context with the account, credential, and environment planes.
 * @param config - Literal key, credential reference, and API base from the settings snapshot.
 * @param route - Provider route of the model request being prepared.
 * @returns Whether the account token or an API key is available to that operation.
 */
export declare function deepSeekWebCredentialReady(ctx: Context, config: {
    apiKey: string | undefined;
    apiKeyEnv: string;
    baseURL: string;
}, route: string | undefined): Promise<boolean>;
/**
 * Check whether an authentication source exists; does not resolve credentials.
 * @param auth - Sources for the operation.
 * @returns Whether a literal key or resolver is present.
 */
export declare function hasDeepSeekWebCredential(auth: DeepSeekWebAuth): boolean;
/** Narrow a remote JSON object.
 * @param value - Untrusted JSON.
 * @returns Whether readable object fields exist.
 */
export declare function isRecord(value: unknown): value is Record<string, unknown>;
/** Report malformed upstream JSON without embedding response content.
 * @returns The stable provider error.
 */
export declare function invalidResponse(): WebError;
/** Dispatch one operation using a single settings and credential snapshot.
 * @param options - Authentication sources for this operation.
 * @param request - Resolved endpoint, JSON payload, and response decoder.
 * @param signal - Caller cancellation signal.
 * @returns The validated provider result.
 */
export declare function dispatchDeepSeekWeb<T>(options: DeepSeekWebAuth, request: DeepSeekWebRequest<T>, signal?: AbortSignal): Promise<T>;
export {};
//# sourceMappingURL=transport.d.ts.map