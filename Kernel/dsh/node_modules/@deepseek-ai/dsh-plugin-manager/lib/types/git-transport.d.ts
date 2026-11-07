import type { ParsedInstallSpec } from './install-spec.ts';
/** Deadline and cancellation for each read-only access probe. */
export interface GitTransportOptions {
    timeoutMs: number;
    signal: AbortSignal;
    /** The application-owned package manager's environment, also used by its Git children. */
    env?: Readonly<Record<string, string>>;
}
/**
 * The SSH form of an HTTP(S) Git spec on a host pnpm has no transport fallback for. pnpm tries HTTPS and SSH itself
 * for the hosts `hosted-git-info` knows, such as GitHub and GitLab.com, and clones any other URL over the transport
 * it names.
 * @param spec The parsed installation address.
 * @returns `git+ssh://git@<host>/<path>` on SSH's default port with the spec's fragment, or `undefined` for a spec that
 * is not an HTTP(S) Git URL on such a host, or whose URL carries credentials or a query.
 */
export declare function sshGitSpec(spec: ParsedInstallSpec): string | undefined;
/**
 * Choose how pnpm clones an HTTP(S) Git repository that {@link sshGitSpec} has an SSH form for: probe the spec's URL,
 * and when HTTP(S) cannot read it, probe the SSH form.
 * @param spec The parsed installation address.
 * @param dir The profile directory where installation runs.
 * @param options The deadline of each probe, cancellation and the installer's environment.
 * @returns The SSH form when only SSH can read the repository; otherwise the spec unchanged.
 */
export declare function chooseGitTransport(spec: ParsedInstallSpec, dir: string, options: GitTransportOptions): Promise<string>;
//# sourceMappingURL=git-transport.d.ts.map