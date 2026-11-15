/**
 * Locate an engine package manifest for Kit.
 * @param name - Engine package name requested by Kit.
 * @returns Absolute path of the package's `package.json`.
 * @throws An error with code `MODULE_NOT_FOUND` when the package is absent.
 */
export type EnginePackageResolver = (name: string) => string;
/**
 * Locate the Kit entry the consumer imports, normally
 * `() => import.meta.resolve('@deepseek-ai/libreoffice-kit')` written in the consuming module.
 * @returns The Kit entry as a file URL string or `URL`.
 * @throws When the consumer cannot resolve Kit; the resolver treats every thrown value as an absent Kit.
 */
export type KitEntryLookup = () => string | URL;
/**
 * Kit module location for a consumer that imports Kit at run time.
 * `available` carries the Kit entry file URL for a dynamic `import()` and an engine resolver anchored at that entry;
 * `missing` carries the lookup failure.
 */
export type KitModuleResolution = {
    readonly kind: 'available';
    readonly entry: string;
    readonly resolveEnginePackage: EnginePackageResolver;
} | {
    readonly kind: 'missing';
    readonly error: unknown;
};
/**
 * Locate Kit for a consumer that loads it with a dynamic `import()`.
 * @param lookupKitEntry - Consumer's Kit lookup; anchoring at the consumer, not this library, selects the installation the consumer uses.
 * @returns `available` with the entry file URL and an engine resolver created by {@link createEnginePackageResolver},
 * or `missing` when the lookup throws.
 * @throws Node's error if a returned entry cannot be used by `createRequire`.
 */
export declare function resolveKitModule(lookupKitEntry: KitEntryLookup): KitModuleResolution;
/**
 * Anchor engine resolution to the Kit entry actually used by the consumer, not this package's installation.
 * Each returned callback retains its own Node resolver and installs no module hooks.
 * @param kitEntry - Absolute Kit entry path or file URL, normally the consumer's `import.meta.resolve('@deepseek-ai/libreoffice-kit')`.
 * @returns A callback for Kit's `resolveEnginePackage` option. Native paths map the first ASAR segment to its unpacked copy;
 * WASM and paths outside archives stay unchanged. Kit validates the returned manifest and required engine assets.
 * @throws Node's error if `kitEntry` cannot be used by `createRequire`.
 * On invocation, the callback preserves Node errors for absent packages and other resolution failures.
 * An installed package with an unresolvable manifest throws an incomplete-installation Error with the Node error as its cause,
 * preventing engine fallback. Filesystem inspection errors propagate unchanged.
 */
export declare function createEnginePackageResolver(kitEntry: string | URL): EnginePackageResolver;
//# sourceMappingURL=locate.d.ts.map