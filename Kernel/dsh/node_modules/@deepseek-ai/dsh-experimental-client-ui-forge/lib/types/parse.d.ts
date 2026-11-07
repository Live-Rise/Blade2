/** Parsers for the porcelain output of the read-only Git commands the panel runs. */
/** One remote with the URLs `git remote -v` prints for it. */
export interface GitRemote {
    /** Remote name such as `origin`. */
    readonly name: string;
    /** Distinct URLs, fetch URL first. */
    readonly urls: readonly string[];
}
/**
 * Read the first line that `git rev-parse` prints for a directory query such as `--show-toplevel`.
 * @param output - stdout of the command.
 * @returns the directory.
 * @throws when the output is empty.
 */
export declare function parseTopLevel(output: string): string;
/**
 * Read the first line of a one-name output such as `git symbolic-ref --short HEAD`.
 * @param output - stdout of the command.
 * @returns the name, or null when the command printed none.
 */
export declare function parseName(output: string): string | null;
/**
 * Order worktree base branches: the checked-out branch first, then the other
 * listed branches in listed order.
 * @param current - checked-out branch, or null when HEAD is detached.
 * @param output - stdout of `git for-each-ref --format=%(refname:lstrip=2) refs/heads/`.
 * @param limit - most names to return.
 * @returns distinct branch names, at most `limit`.
 */
export declare function baseBranches(current: string | null, output: string, limit: number): string[];
/**
 * Group the lines of `git remote -v` by remote.
 * @param output - stdout of the command.
 * @returns remotes in first-listed order.
 */
export declare function parseRemotes(output: string): GitRemote[];
/**
 * Split an upstream branch such as `origin/feature/x` into its remote and the
 * branch on that remote. The remote is the longest remote name followed by `/`
 * at the start of the upstream branch.
 * @param upstream - upstream branch such as `origin/main`, or null.
 * @param remotes - remotes in listed order.
 * @returns the remote and branch, or undefined without an upstream or a remote it starts with.
 */
export declare function upstreamBranch(upstream: string | null, remotes: readonly GitRemote[]): {
    readonly remote: GitRemote;
    readonly branch: string;
} | undefined;
/**
 * Order remotes for code host recognition: the upstream's remote first, then
 * `origin`, then the rest in listed order.
 * @param upstream - upstream branch such as `origin/main`, or null.
 * @param remotes - remotes in listed order.
 * @returns the same remotes in recognition order.
 */
export declare function remoteOrder(upstream: string | null, remotes: readonly GitRemote[]): GitRemote[];
//# sourceMappingURL=parse.d.ts.map