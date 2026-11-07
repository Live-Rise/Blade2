import { type DetailFiles } from '@deepseek-ai/dsh-plugin-detail';
import { type RepositoryFileOptions, type RepositoryRef } from './detail-files.ts';
import type { PluginDetail } from './types.ts';
/** A repository directory whose README is the last detail source. */
export interface RepositoryLocation extends RepositoryRef {
    /** Package directory inside the repository, `''` for its root. */
    readonly directory: string;
}
/** Where one package's detail sources are. */
export interface DetailLocation {
    /** The package's files, when they can be read; `locale/<language>.json` and the README are read from them. */
    readonly files?: DetailFiles;
    /** Package directory inside `files`, `''` for its root. */
    readonly dir: string;
    /**
     * The repository read last, at its default branch; `manifest` reads `repository` from the package's `package.json`
     * in `files`.
     */
    readonly repository: RepositoryLocation | 'manifest' | undefined;
}
/** Limits of detail documents and their images. */
export interface DetailLimits {
    readonly maxDocumentBytes: number;
    readonly maxImageBytes: number;
    /** Lifetime of a read result, including a failure that a retry would repeat. */
    readonly ttlMs: number;
}
/**
 * The repository a URL or shorthand names: hosts npm knows by their owner and project, so a GitHub page such as
 * `/o/r/tree/main/pkg` names `o/r`; other http(s) URLs by their path, without a GitLab `/-/` page suffix.
 * @param url - a manifest `repository` URL or shorthand, or a market index's `repository.url`.
 * @returns the repository, or undefined for a gist, another protocol, or fewer than two path segments.
 */
export declare function repositoryOf(url: string): RepositoryRef | undefined;
/**
 * Parse a manifest `repository` into the repository and directory its README is read from.
 * @param value - a manifest `repository` string or object.
 * @returns the location, or undefined when it names no repository {@link repositoryOf} accepts; a `directory` that
 *   leaves the repository is dropped.
 */
export declare function manifestRepository(value: unknown): RepositoryLocation | undefined;
/** Reads and caches detail documents for one service. */
export declare class DetailReader {
    private readonly limits;
    private readonly repositories;
    private readonly results;
    private readonly pending;
    private readonly documents;
    /**
     * @param limits - document and image sizes and the result lifetime.
     * @param repositories - single-file API, clone, and request settings of repository reads.
     */
    constructor(limits: DetailLimits, repositories: RepositoryFileOptions);
    /**
     * Read a package's detail document, sharing concurrent reads and reusing a result, or a failure that reading again
     * would repeat, for `ttlMs`.
     * @param location - the package's sources.
     * @param languages - the chain from `languageChain` of `dsh-plugin-detail`.
     * @param refresh - read again instead of reusing a cached result.
     * @returns the first document the sources provide, or `none`.
     * @throws {DetailFailure} when a source that should provide a document cannot be read.
     */
    read(location: DetailLocation, languages: readonly string[], refresh?: boolean): Promise<PluginDetail>;
    /**
     * Read an image beside a document.
     * @param id - the document id a detail read returned.
     * @param authored - the image destination as the document writes it: relative to the document, or to the package or
     *   repository root when it starts with `/`; a query or fragment is ignored.
     * @returns the bytes and media type, or undefined for an unknown document, a path outside its root, an unsupported
     *   image type, or a missing file.
     * @throws {DetailFailure} when the source fails or the image exceeds `maxImageBytes`.
     */
    image(id: string, authored: string): Promise<{
        bytes: Uint8Array;
        mediaType: string;
    } | undefined>;
    /**
     * Mark a document used and forget documents unused for `ttlMs`, so ids live as long as a read or an image request
     * returns them. A cached read is younger than `ttlMs` and marks its document, so no cached read names a forgotten id.
     */
    private use;
    /** Cache a read and drop expired ones, so the cache holds only reads younger than `ttlMs`. */
    private remember;
    private resolve;
    private manifestRepositoryOf;
    private found;
}
//# sourceMappingURL=detail.d.ts.map