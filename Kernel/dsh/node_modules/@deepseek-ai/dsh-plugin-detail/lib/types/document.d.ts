import { type DetailFiles } from './files.ts';
/**
 * The languages a document is chosen in: the language, each shorter prefix, then English.
 * @param language - a BCP 47 language id such as `zh-CN`.
 * @returns lower-case language ids, most specific first.
 * @throws when `language` is not a language id.
 */
export declare function languageChain(language: string): string[];
/**
 * A locale `meta.detail` value: a `/`-separated Markdown path relative to the package root that cannot leave it.
 * @param value - the declared `meta.detail` value, or `undefined` when the locale file omits it.
 * @param label - the locale file path used in a failure message.
 * @returns the validated path, or `undefined` when the value is absent.
 * @throws {DetailFailure} `invalid` for another value.
 */
export declare function detailPath(value: unknown, label: string): string | undefined;
/**
 * Decode a Markdown document: UTF-8 without a byte order mark, without a leading YAML front matter block, which is file
 * metadata rather than description text.
 * @param bytes - the file's bytes.
 * @returns the Markdown text.
 */
export declare function markdownOf(bytes: Uint8Array): string;
/** A package's detail document. */
export interface PackageDocument {
    /** `locale` for the file `meta.detail` names, `readme` for the package's README. */
    readonly origin: 'locale' | 'readme';
    /** `/`-separated path of the document inside the files. */
    readonly path: string;
    /** The document, decoded by {@link markdownOf}. */
    readonly markdown: string;
}
/**
 * Read a package's detail document: the Markdown file that `meta.detail` in its `locale/<language>.json` names for the
 * first language of the chain that declares one, otherwise its README.
 * @param files - the package's files.
 * @param dir - package directory inside `files`, `''` for its root.
 * @param languages - the chain from {@link languageChain}.
 * @param maxBytes - largest accepted locale file and document.
 * @param signal - cancellation of the reads.
 * @returns the document, or undefined when the package has neither.
 * @throws {DetailFailure} `invalid` for a malformed locale file or `meta.detail` or a named file that does not exist, and
 *   the failures of `files`.
 */
export declare function readPackageDocument(files: DetailFiles, dir: string, languages: readonly string[], maxBytes: number, signal: AbortSignal): Promise<PackageDocument | undefined>;
/**
 * Resolve an image destination a document writes.
 * @param document - `/`-separated path of the document inside its files.
 * @param authored - the destination as written: relative to the document, or to the root of its files when it starts
 *   with `/`; a query or fragment is ignored.
 * @returns the image's path inside the files and its media type, or undefined for a malformed destination, a path that
 *   leaves the root, or a type other than PNG, JPEG, GIF, WebP, AVIF, and SVG.
 */
export declare function detailImage(document: string, authored: string): {
    path: string;
    mediaType: string;
} | undefined;
//# sourceMappingURL=document.d.ts.map