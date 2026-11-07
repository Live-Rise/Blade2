/**
 * Normalize a public Relay base URL by removing query, hash, and trailing slashes.
 * @param value Public Relay URL or base URL.
 * @returns Canonical base URL without credentials, query, hash, or trailing slash.
 */
export declare function normalizeRelayUrl(value: string): string;
/**
 * Split one Relay base URL into its origin and base path.
 * @param relay Public Relay URL or base URL.
 * @returns Validated origin and trailing-slash-free base path.
 */
export declare function relayEndpoint(relay: string): {
    origin: string;
    basePath: string;
};
//# sourceMappingURL=relay-url.d.ts.map