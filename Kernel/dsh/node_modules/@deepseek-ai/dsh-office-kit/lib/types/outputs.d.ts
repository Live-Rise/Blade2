/** Limits and location of the output store. */
export interface OutputStoreOptions {
    /** Absolute directory owned by the store; created on first save with owner-only permissions. */
    readonly root: string;
    /** Milliseconds a saved file is kept before deletion. */
    readonly retentionMs: number;
    /** Inclusive bytes across retained files; older files are deleted first to make room. */
    readonly maxBytes: number;
}
/**
 * Keeps each saved output in its own private directory under a per-owner directory. Files leave when their retention
 * expires, when newer files need the byte budget, when their owner is released, and when the store is disposed. A file
 * counts toward the byte budget until its deletion succeeds; a failed deletion is reported and retried by later
 * eviction, release, and disposal. Only files this store saved are deleted, apart from a startup sweep of expired
 * leftovers of earlier processes. The sweep considers only real directories, never links or junctions, whose names this
 * store generates: `session-` and 24 lowercase hexadecimal digits under the root, and a lowercase UUID v4 below that.
 */
export declare class OutputStore {
    private readonly options;
    private readonly warn;
    private readonly entries;
    private bytes;
    private disposed;
    private readonly sweep;
    private saving;
    /** Deletions and releases in progress, awaited by disposal. */
    private readonly pending;
    /**
     * @param options - root directory, retention, and byte budget.
     * @param warn - receives contained cleanup failures.
     */
    constructor(options: OutputStoreOptions, warn: (message: string) => void);
    /**
     * Save one output for an owner.
     * @param owner - opaque owner identity, such as a Session id.
     * @param name - file name shown to the consumer; only its final path component is used.
     * @param data - complete output bytes.
     * @returns the absolute path of the saved file.
     * @throws {OfficeKitError} `output-too-large` when the file alone exceeds the byte budget, and `unavailable` after
     * disposal or when retained files whose deletion failed leave too little of the budget.
     */
    save(owner: string, name: string, data: Uint8Array): Promise<string>;
    /** Saves run one at a time so the byte budget covers every retained file. */
    private saveNow;
    /**
     * Delete every file saved for an owner. Disposal waits for a release in progress.
     * @param owner - owner whose files are released.
     * @returns completion after deletion settles; failed deletions are reported and retained for retry.
     */
    release(owner: string): Promise<void>;
    private releaseNow;
    /**
     * Refuse further saves, wait for deletions and releases in progress, and delete every file this store saved,
     * retrying earlier failed deletions once more.
     * @returns completion after deletion settles; files that still cannot be deleted are reported.
     */
    dispose(): Promise<void>;
    /** Wait until no deletion or release is in progress, including ones started while waiting. */
    private settle;
    /** Record a non-rejecting deletion or release as in progress until it settles. */
    private track;
    /**
     * Delete one saved file, sharing a deletion already in progress. The entry and its bytes leave the store only after
     * the deletion succeeds; a failure is reported and leaves the entry for a later retry.
     * @returns whether the file is gone.
     */
    private remove;
    private removeEmpty;
    /**
     * Delete output directories, including those of earlier processes, whose files are older than the retention. Only
     * real directories with store-generated names are considered; links, junctions, and other entries stay untouched.
     */
    private sweepExpired;
}
//# sourceMappingURL=outputs.d.ts.map