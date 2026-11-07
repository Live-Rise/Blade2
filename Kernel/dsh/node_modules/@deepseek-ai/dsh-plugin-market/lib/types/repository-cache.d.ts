import { type FoundFile } from '@deepseek-ai/dsh-plugin-detail';
/** Limits, location, and transports of the clone cache. */
export interface RepositoryCacheOptions {
    /** Directory holding one bare repository per repository URL. */
    readonly dir: string;
    /** Largest amount one command adds to a repository; a command that adds more is stopped and the repository deleted. */
    readonly maxRepositoryBytes: number;
    /** Largest total size; least recently used repositories are deleted after a fetch until the cache fits. */
    readonly maxCacheBytes: number;
    /** Deadline of one Git command and of one anonymous accessibility check. */
    readonly timeoutMs: number;
    /** How long a repository's default-branch commit and its anonymous accessibility are reused. */
    readonly ttlMs: number;
    /** Git transports Git may use, such as `https` and `ssh`; `url.<base>.insteadOf` in the user's configuration still applies. */
    readonly transports: readonly string[];
    /** Disposal of the owning service. */
    readonly lifetime: AbortSignal;
    /** Interval between size checks of a repository while Git fetches into it. */
    readonly sizePollMs: number;
}
/** Partial clones of detail repositories under one directory, owned by one service. */
export declare class RepositoryCache {
    private readonly options;
    private readonly locks;
    private readonly heads;
    /**
     * Whether `git --version` has succeeded. A missing Git is checked this way before any operation, because Windows
     * reports a missing command as an ordinary exit code that would otherwise read as a refused fetch.
     */
    private gitStarts;
    private readonly visibility;
    private readonly checks;
    private readonly running;
    /** @param options - location, limits, deadline, transports, and lifetime. */
    constructor(options: RepositoryCacheOptions);
    /**
     * Whether a repository's smart HTTP endpoint answers without credentials, which makes a missing single-file API answer
     * authoritative. The request carries no user configuration, credential helper, or `.netrc` entry; concurrent checks of
     * one repository share it, and its answer is reused for `ttlMs`.
     * @param url - the repository's https URL, ending in `.git`.
     * @returns true for a 2xx answer; false for 401, 403, 404, or a redirect, which hosts send for a private repository.
     * @throws {DetailFailure} `network` when the host cannot be reached or answers another status.
     */
    isPublic(url: string): Promise<boolean>;
    private check;
    /**
     * Read one regular file of a repository.
     * @param url - the repository URL.
     * @param commit - a full commit id, or undefined for the default branch.
     * @param path - `/`-separated path relative to the repository root.
     * @param maxBytes - largest accepted size.
     * @param signal - cancellation of the read.
     * @returns the bytes, or undefined when the commit has no regular file at `path`.
     * @throws {DetailFailure} when Git cannot fetch the commit, the repository grows past its limit, or the file exceeds `maxBytes`.
     */
    read(url: string, commit: string | undefined, path: string, maxBytes: number, signal: AbortSignal): Promise<Uint8Array | undefined>;
    /**
     * Read the README of a repository directory: `README.md`, `README.markdown`, or `README` in any letter case.
     * @param url - the repository URL.
     * @param commit - a full commit id, or undefined for the default branch.
     * @param dir - directory relative to the repository root; `''` for the root.
     * @param maxBytes - largest accepted size.
     * @param signal - cancellation of the read.
     * @returns the file, or undefined when the directory has none.
     * @throws {DetailFailure} as {@link read} does.
     */
    readme(url: string, commit: string | undefined, dir: string, maxBytes: number, signal: AbortSignal): Promise<FoundFile | undefined>;
    /** Wait until every running Git command has ended; the lifetime signal stops them. */
    settled(): Promise<void>;
    /**
     * Serialize operations on one repository, so a fetch and the reads after it never interleave, and delete the
     * repository when an operation fails or is cancelled for any reason but a file over its read's size.
     */
    private locked;
    /**
     * Run an operation after every earlier operation on the same repository directory has ended, in this service and in
     * other processes that share the cache directory, such as Web and Desktop on one profile.
     */
    private serialized;
    /** The commit to read: a fetched commit, fetching it or the default branch when necessary. */
    private commit;
    /** Fetch one commit without file contents. */
    private fetch;
    /**
     * Run a Git command that may download objects, stopping it when the repository grows past its limit during or by the
     * command; the caller's lock then deletes the repository.
     */
    private watched;
    /**
     * Delete least recently used repositories until the cache fits, `kept` last, and staging directories older than the
     * Git deadline. Each deletion is queued behind the operations on its repository and not awaited, because the caller
     * holds the lock of `kept` and another repository's queued operation may be waiting to evict `kept`.
     */
    private evict;
    /**
     * The regular files directly below a tree path, or the file at a path, with their blob ids; listing reads only trees,
     * so it fetches no file contents.
     */
    private files;
    /** Read a blob, fetching its contents under the repository size limit when the clone does not hold them yet. */
    private blob;
    /**
     * Run a Git command on local objects and return its output. A failure, such as a missing Git, a full disk, or a timeout,
     * is a plain error: it says nothing about the repository, so no read caches it.
     */
    private run;
    /**
     * Run Git with options that keep it from running repository-supplied programs and allow only the configured transports.
     * Cancellation by `signal` or the lifetime rejects with its reason; `stop` ends Git with a failed result.
     */
    private git;
}
//# sourceMappingURL=repository-cache.d.ts.map