import type { DetailFailureReason, NpmFileSource } from './types.ts';
/** A read that must not fall back: the source exists but refused, failed, or exceeded a limit. */
export declare class DetailFailure extends Error {
    readonly reason: DetailFailureReason;
    /**
     * @param reason - the failure kind a reader reports.
     * @param message - the diagnostic, naming the source.
     * @param options - the underlying error.
     */
    constructor(reason: DetailFailureReason, message: string, options?: ErrorOptions);
}
/** One file found by name lookup. */
export interface FoundFile {
    /** `/`-separated path relative to the source root. */
    readonly path: string;
    readonly bytes: Uint8Array;
}
/** Files below one root, read by `/`-separated relative paths that never start with `/` or contain `.` or `..` segments. */
export interface DetailFiles {
    /** Identity of the files' content: two sources with the same key read the same bytes. */
    readonly key: string;
    /**
     * Read one regular file.
     * @param path - path relative to the root.
     * @param maxBytes - largest accepted size.
     * @param signal - cancellation of the read.
     * @returns the file's bytes, or undefined when it does not exist.
     * @throws {DetailFailure} when the source fails or the file exceeds `maxBytes`.
     */
    read(path: string, maxBytes: number, signal: AbortSignal): Promise<Uint8Array | undefined>;
    /**
     * Read the README of a directory: `README.md`, `README.markdown`, or `README` in any letter case.
     * @param dir - directory relative to the root; `''` for the root.
     * @param maxBytes - largest accepted size.
     * @param signal - cancellation of the read.
     * @returns the file, or undefined when the directory has none.
     * @throws {DetailFailure} when the source fails or the file exceeds `maxBytes`.
     */
    readme(dir: string, maxBytes: number, signal: AbortSignal): Promise<FoundFile | undefined>;
}
/** README names tried in order where a source cannot list a directory. */
export declare const README_NAMES: readonly string[];
/**
 * Join a directory and a file name into a root-relative path.
 * @param dir - directory relative to the root, `''` for the root.
 * @param name - path relative to `dir`.
 * @returns the joined path.
 */
export declare function joinPath(dir: string, name: string): string;
/**
 * Pick the README entry of a directory listing by the order of {@link README_NAMES}, then by name.
 * @param names - file names of the directory.
 * @returns the README name, or undefined.
 */
export declare function readmeName(names: readonly string[]): string | undefined;
/** Reads one file: its bytes, or undefined when it is absent. */
export type ReadFile = (path: string, maxBytes: number, signal: AbortSignal) => Promise<Uint8Array | undefined>;
/**
 * Read the first README of `names` that exists, for sources that cannot list a directory.
 * @param read - reads one file.
 * @param dir - directory relative to the root; `''` for the root.
 * @param maxBytes - largest accepted size.
 * @param signal - cancellation of the reads.
 * @param names - the names to try, in order.
 * @returns the file, or undefined when none exists.
 * @throws {DetailFailure} what `read` throws.
 */
export declare function readmeByName(read: ReadFile, dir: string, maxBytes: number, signal: AbortSignal, names?: readonly string[]): Promise<FoundFile | undefined>;
/**
 * Files of a local package directory. A path whose real location leaves the directory reads as absent.
 * @param root - absolute package directory.
 * @param revision - the installed version, so a reinstalled package reads as different files.
 * @returns the files.
 */
export declare function localFiles(root: string, revision: string): DetailFiles;
/** Request settings of a remote file source. */
export interface RemoteFileOptions {
    readonly timeoutMs: number;
    /** Cancellation that outlives one read, such as the owning service's disposal. */
    readonly lifetime: AbortSignal;
}
/** One HTTP answer: the body of a 2xx response, or the status of any other. */
export type FetchedBody = {
    readonly ok: true;
    readonly bytes: Uint8Array;
} | {
    readonly ok: false;
    readonly status: number;
};
/**
 * GET a URL, following redirects within its origin, and read the body up to `maxBytes`.
 * @param url - the request URL.
 * @param maxBytes - largest accepted body.
 * @param signal - cancellation of the read.
 * @param options - deadline and service lifetime.
 * @returns the body, or the status of a non-2xx response, including a redirect to another origin.
 * @throws {DetailFailure} `network` when no response arrives or the body fails, `too-large` over `maxBytes`.
 */
export declare function fetchBody(url: string, maxBytes: number, signal: AbortSignal, options: RemoteFileOptions): Promise<FetchedBody>;
/** Every npm file CDN, in the default order. */
export declare const NPM_FILE_SOURCES: readonly ["jsdelivr", "unpkg", "npmmirror"];
/**
 * Order the configured npm file CDNs by a caller's preference: the preferred ones that are configured, in the preferred
 * order, then the other configured ones in configured order. A preferred CDN that is not configured is not asked.
 * @param configured - the configured CDNs in order.
 * @param preferred - CDNs to ask first, such as those a Client measured as fastest; names other than CDNs are ignored.
 * @returns the CDNs to ask, first to last.
 */
export declare function npmFileSourceOrder(configured: readonly NpmFileSource[], preferred?: readonly string[]): NpmFileSource[];
/**
 * Percent-encode each segment of a `/`-separated path for a URL.
 * @param path - the path.
 * @returns the encoded path.
 */
export declare function encodePath(path: string): string;
/**
 * Files of one published npm package version, read from public file CDNs in order. A 404 from a CDN means the package
 * does not contain the file; a network failure or another status asks the next CDN.
 * @param name - package name.
 * @param version - exact version.
 * @param sources - the CDNs to ask, first to last; never empty.
 * @param options - deadline and service lifetime.
 * @returns the files.
 */
export declare function npmFiles(name: string, version: string, sources: readonly NpmFileSource[], options: RemoteFileOptions): DetailFiles;
//# sourceMappingURL=files.d.ts.map