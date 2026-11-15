import { type ParsedInstallSpec } from './install-spec.ts';
import type { PackageResult } from './types.ts';
/**
 * A failed check, with the Git command line the install dialog shows as the installation's only run.
 * The returned result itself stays a plain {@link PackageResult}: the command line never joins the wire fields.
 */
export interface GithubConnectionFailure extends PackageResult {
    /**
     * The `git` command line the check ran, with the user information of an http(s) repository URL removed:
     * `git -c credential.helper= ls-remote -- <repository> HEAD`.
     */
    readonly argv: readonly string[];
}
/** Deadline, diagnostics and cancellation for one read-only Git repository check. */
export interface GithubConnectionOptions {
    timeoutMs: number;
    outputBytes: number;
    signal: AbortSignal;
    /** The application-owned package manager's environment, also used by its Git children. */
    env?: Readonly<Record<string, string>>;
}
/**
 * Check a GitHub repository before pnpm starts, without downloading or building its package.
 * @param spec The parsed installation address; npm packages, local paths and other hosts are not checked.
 * @param dir The profile directory where installation runs.
 * @param options The connection deadline, output bound and operation cancellation.
 * @returns As {@link checkGitConnection}, or undefined for a spec that does not name a GitHub repository.
 */
export declare function checkGithubConnection(spec: ParsedInstallSpec, dir: string, options: GithubConnectionOptions): Promise<GithubConnectionFailure | undefined>;
/**
 * Check a Git repository with `git ls-remote`, without downloading or building its package. A host shorthand is read
 * over HTTPS; any other spec over the URL it names. Git reads the profile's Git and proxy configuration without
 * invoking credential helpers or prompting, so a repository that needs credentials fails as `unknown`, not as a
 * connection failure. Timeout and cancellation terminate its descendants too.
 * @param spec The parsed address of a Git repository.
 * @param dir The profile directory where installation runs.
 * @param options The connection deadline, output bound and operation cancellation.
 * @returns A failed check carrying the command line it ran (without URL user information), its bounded output and its
 * failure kind, with a complete log at logPath, or undefined for a reachable repository.
 */
export declare function checkGitConnection(spec: ParsedInstallSpec & {
    readonly kind: 'git';
}, dir: string, options: GithubConnectionOptions): Promise<GithubConnectionFailure | undefined>;
//# sourceMappingURL=github-connection.d.ts.map