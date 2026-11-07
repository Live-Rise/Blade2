/**
 * Package-asset format shared by the client build preset, the Host asset
 * route, and repository gates: the supported media types, the export that
 * publishes built assets, the content revision, and the document-relative
 * browser reference. Changing any value here changes a published URL or
 * package-manifest requirement, so the preset, the route, and the gates read
 * it from this one module.
 * @module @deepseek-ai/dsh-client-modules/src/assets
 */
/** Route segment between a package name and its asset path. */
export declare const PACKAGE_ASSET_SEGMENT = "assets";
/** Media type served for each supported asset extension, keyed by the lowercase extension with its dot. */
export declare const PACKAGE_ASSET_MEDIA_TYPES: ReadonlyMap<string, string>;
/** Package export key that publishes built assets. */
export declare const PACKAGE_ASSET_EXPORT = "./assets/*";
/** The only accepted target of {@link PACKAGE_ASSET_EXPORT}. */
export declare const PACKAGE_ASSET_EXPORT_TARGET = "./lib/assets/*";
/** The `files` entry that publishes {@link PACKAGE_ASSET_EXPORT_TARGET}. */
export declare const PACKAGE_ASSET_FILES_ENTRY = "lib/assets";
/**
 * Look up the media type of an asset path.
 * @param assetPath - file path or URL path; only its final extension is read, case-insensitively.
 * @returns the served media type, or undefined when the extension is unsupported.
 */
export declare function packageAssetMediaType(assetPath: string): string | undefined;
/**
 * Compute the content revision of one built asset. The build writes it into
 * the emitted file name, so a URL names exactly one content and a rebuilt
 * asset gets a new URL.
 * @param bytes - exact bytes written under `lib/assets/`.
 * @returns the first 12 hexadecimal digits of the SHA-1 digest.
 */
export declare function packageAssetRevision(bytes: Uint8Array): string;
/**
 * Name the emitted file of one asset by inserting its revision before the extension.
 * @param assetPath - `/`-separated path below `src/client/assets/`, such as `icons/whale@2x.png`.
 * @param rev - {@link packageAssetRevision} of the asset bytes.
 * @returns the path below `lib/assets/`, such as `icons/whale@2x-<rev>.png`.
 */
export declare function packageAssetRevisionedPath(assetPath: string, rev: string): string;
/**
 * Build the browser reference to one built asset. The reference is relative
 * to the shell document, like every other app-owned route, so it resolves
 * under any document mount and from injected `<style>` text.
 * @param packageName - owning package name, emitted verbatim; a scoped name keeps its `/`.
 * @param emittedPath - {@link packageAssetRevisionedPath} result below `lib/assets/`; each segment is
 *   percent-encoded, including `!`, `'`, `(`, `)`, and `*`, so the reference is also a valid unquoted CSS `url()`.
 * @returns `plugins/<packageName>/assets/<path>`.
 */
export declare function packageAssetReference(packageName: string, emittedPath: string): string;
//# sourceMappingURL=assets.d.ts.map