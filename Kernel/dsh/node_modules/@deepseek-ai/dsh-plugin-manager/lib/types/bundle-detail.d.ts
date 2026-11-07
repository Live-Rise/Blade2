import type { ProfileContext } from '@deepseek-ai/dsh-app-boot';
import { type NpmFileSource } from '@deepseek-ai/dsh-plugin-detail';
import type { BundleDetail, BundleInfo } from './types.ts';
/** Limits and sources of bundle detail reads. */
export interface BundleDetailOptions {
    readonly profile: ProfileContext;
    readonly maxDocumentBytes: number;
    readonly maxImageBytes: number;
    readonly npmFileSources: readonly NpmFileSource[];
    /** Deadline of one CDN request. */
    readonly timeoutMs: number;
    /** The plugin manager's disposal. */
    readonly lifetime: AbortSignal;
}
/**
 * Read a bundle's detail document in a display language.
 * @param bundle - the bundle as `listBundles()` reports it.
 * @param language - a BCP 47 language id.
 * @param preferred - npm file CDNs to ask first for an offered version; see `npmFileSourceOrder`.
 * @param options - profile, sources, and limits.
 * @returns the document, or `none` when the bundle has neither a `meta.detail` file nor a README.
 * @throws {RemoteError} `plugin-manager/invalid-language`, `plugin-manager/unknown-bundle`, or
 *   `plugin-manager/detail-unavailable`.
 */
export declare function readBundleDocument(bundle: BundleInfo, language: string, preferred: readonly string[], options: BundleDetailOptions): Promise<BundleDetail>;
/**
 * Read an image beside a bundle's detail document.
 * @param bundle - the bundle as `listBundles()` reports it.
 * @param document - the document's path inside the bundle, as {@link readBundleDocument} returned it.
 * @param destination - the image destination as the document writes it.
 * @param preferred - npm file CDNs to ask first for an offered version; see `npmFileSourceOrder`.
 * @param options - profile, sources, and limits.
 * @returns the bytes and media type, or undefined for a path outside the bundle, an unsupported type, or a missing file.
 * @throws {RemoteError} `plugin-manager/unknown-bundle` or `plugin-manager/detail-unavailable`.
 */
export declare function readBundleImage(bundle: BundleInfo, document: string, destination: string, preferred: readonly string[], options: BundleDetailOptions): Promise<{
    bytes: Uint8Array;
    mediaType: string;
} | undefined>;
//# sourceMappingURL=bundle-detail.d.ts.map