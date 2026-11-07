/** GitHub remote URL and pull request URL parsing, and validation of the parts inlined into requests. */
import type { ChangeRequestRef, ForgeRepository } from '@deepseek-ai/dsh-experimental-forge';
/** Host, owner, and repository name addressed by a URL. */
export interface GitHubLocation {
    /** Lowercase host name. */
    readonly host: string;
    /** Owner as the URL spells it. */
    readonly owner: string;
    /** Repository name without a `.git` suffix. */
    readonly name: string;
}
/**
 * Check one owner or repository name segment.
 * @param value - candidate segment.
 * @returns whether GitHub permits the segment and it is not a dot path.
 */
export declare function isSegment(value: string): boolean;
/**
 * Validate a pull request before its owner, name, and number are inlined into
 * a GraphQL document, a `gh` argument, or an API path.
 * @param ref - pull request on a GitHub host.
 * @returns its repository.
 * @throws when the owner or name is not a permitted segment or the number is not a positive integer.
 */
export declare function checkedRepository(ref: ChangeRequestRef): ForgeRepository;
/**
 * Parse a Git remote URL. Accepts `http(s)://[user@]host[:port]/owner/name`,
 * `ssh://[user@]host[:port]/owner/name`, `git://host/owner/name`, and the
 * scp-like `[user@]host:owner/name`, each with an optional `.git` suffix or
 * trailing slash. HTTP(S) ports belong to the host identity; SSH and Git
 * transport ports do not.
 * @param url - fetch or push URL as Git reports it.
 * @returns the location, or undefined for other URL shapes.
 */
export declare function parseRemoteUrl(url: string): GitHubLocation | undefined;
/**
 * Parse a pull request web URL such as `https://github.com/owner/name/pull/12/files`.
 * @param url - web URL of a pull request.
 * @returns the location and number, or undefined for other URLs.
 */
export declare function parsePullRequestUrl(url: string): (GitHubLocation & {
    readonly number: number;
}) | undefined;
//# sourceMappingURL=urls.d.ts.map