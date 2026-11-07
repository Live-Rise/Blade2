/**
 * The `http:` or `https:` URL `value` names, normalized, or null. Login flows open server-supplied URLs with the
 * platform URL handler, which would also launch a `file:` URL or a custom protocol handler. Normalizing
 * percent-encodes control characters, so the URL can also be embedded in terminal hyperlinks.
 */
export declare function trustedHttpUrl(value: unknown): string | null;
//# sourceMappingURL=trusted-url.d.ts.map