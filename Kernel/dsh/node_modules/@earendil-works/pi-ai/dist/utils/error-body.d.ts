export declare const MAX_PROVIDER_ERROR_BODY_CHARS = 4000;
export interface NormalizedProviderError {
    /** HTTP status code, when one could be extracted from the SDK error object. */
    status?: number;
    /** Raw HTTP body reason, already trimmed and truncated to the cap. */
    body?: string;
    /** `error.message`, or `safeJsonStringify(error)` for a non-`Error` throw. */
    message: string;
    /** True when `message` already contains the body (no separate body to add). */
    messageCarriesBody: boolean;
}
export declare function normalizeProviderError(error: unknown): NormalizedProviderError;
/**
 * Connection-level failures carry no status or body, and SDKs replace the
 * reason with a generic message (`openai`: `"Connection error."`). The real
 * reason sits in the `cause` chain, often two levels deep (undici's
 * `"fetch failed"` wraps `connect ECONNREFUSED ...`). Append the deepest
 * cause so callers that surface only the message keep the reason.
 */
export declare function withCauseDetail(error: Error): string;
/**
 * Compose a display string from a normalized error. When the message already
 * carries the body (Anthropic / `@google/genai` happy path) or no body/status
 * was extracted, the message is returned unchanged. Otherwise the status and
 * body are surfaced, with an optional provider prefix.
 *
 * - no prefix: `"<status>: <body>"`
 * - prefix:    `"<prefix> (<status>): <body>"`
 */
export declare function formatProviderError(norm: NormalizedProviderError, prefix?: string): string;
export declare function truncateErrorText(text: string, maxChars: number): string;
export declare function safeJsonStringify(value: unknown): string;
//# sourceMappingURL=error-body.d.ts.map