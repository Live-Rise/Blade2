/**
 * Registers OAuth flow loaders with literal dynamic imports, for Node bundles
 * that split at dynamic imports (esbuild `splitting`, Chord's worker base):
 * each flow lands in a chunk loaded on its first use, inside the bundle.
 * Without it, pi-ai imports the flows through a variable specifier that
 * bundlers cannot follow, so a bundle loads them from pi-ai's own files.
 * Browser bundles must not import this module (the flows are Node-only).
 */
export declare function registerLazyOAuthFlows(): void;
//# sourceMappingURL=lazy-oauth.d.ts.map