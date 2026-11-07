/**
 * Git repository files for plugin detail documents, through GitHub or GitLab single-file APIs with a cached partial
 * clone as the fallback for repositories those APIs cannot serve anonymously.
 */
import { type DetailFiles, type RemoteFileOptions } from '@deepseek-ai/dsh-plugin-detail';
import type { RepositoryCache } from './repository-cache.ts';
/** A repository as single-file APIs and Git address it. */
export interface RepositoryRef {
    /** Lower-case host name, without port. */
    readonly host: string;
    /** The project path below the host, such as `owner/repo` or `group/subgroup/project`, without `.git`. */
    readonly project: string;
    /** URL Git fetches from: `https://<host>/<project>.git`, or the configured http(s) URL as written for other forms. */
    readonly url: string;
}
/** Single-file API settings and the clone fallback of repository sources. */
export interface RepositoryFileOptions extends RemoteFileOptions {
    /** Lower-case hosts serving the GitLab repository files API; `github.com` always uses GitHub's raw file host. */
    readonly gitlabHosts: readonly string[];
    /** The clone cache's reads. */
    readonly cache: Pick<RepositoryCache, 'isPublic' | 'read' | 'readme'>;
}
/**
 * Files of one repository at a commit, or at its default branch. Hosts with a single-file API are read through it; a
 * missing answer is confirmed with an anonymous request to the repository's smart HTTP endpoint, and a repository that
 * needs credentials, or a host without such an API, is read from the cached partial clone.
 * @param repository - the repository.
 * @param commit - a full commit id, or undefined for the default branch.
 * @param options - API hosts, deadline, lifetime, and the clone cache.
 * @returns the files.
 */
export declare function repositoryFiles(repository: RepositoryRef, commit: string | undefined, options: RepositoryFileOptions): DetailFiles;
//# sourceMappingURL=detail-files.d.ts.map