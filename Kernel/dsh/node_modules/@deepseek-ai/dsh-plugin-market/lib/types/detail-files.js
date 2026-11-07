/**
 * Git repository files for plugin detail documents, through GitHub or GitLab single-file APIs with a cached partial
 * clone as the fallback for repositories those APIs cannot serve anonymously.
 */
import { DetailFailure, encodePath, fetchBody, README_NAMES, readmeByName, } from '@deepseek-ai/dsh-plugin-detail';
function apiUrl(api, repository, ref, path) {
    return api === 'github'
        ? `https://raw.githubusercontent.com/${repository.project}/${encodeURIComponent(ref)}/${encodePath(path)}`
        // The repository URL's origin keeps a self-hosted GitLab's scheme and port.
        : `${new URL(repository.url).origin}/api/v4/projects/${encodeURIComponent(repository.project)}/repository/files/${encodeURIComponent(path)}/raw?ref=${encodeURIComponent(ref)}`;
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
export function repositoryFiles(repository, commit, options) {
    const api = repository.host === 'github.com' ? 'github' : options.gitlabHosts.includes(repository.host) ? 'gitlab' : undefined;
    const ref = commit ?? 'HEAD';
    /** An API read: the bytes, or undefined for an answer that does not distinguish a missing file from a private repository. */
    const fromApi = async (path, maxBytes, signal, via) => {
        const body = await fetchBody(apiUrl(via, repository, ref, path), maxBytes, signal, options);
        if (body.ok)
            return body.bytes;
        if (body.status !== 404 && body.status !== 401 && body.status !== 403) {
            throw new DetailFailure('network', `${repository.url}: ${path} answered HTTP ${String(body.status)}`);
        }
        return undefined;
    };
    return {
        key: `git:${repository.url}#${ref}`,
        async read(path, maxBytes, signal) {
            if (api !== undefined) {
                const bytes = await fromApi(path, maxBytes, signal, api);
                if (bytes !== undefined || await options.cache.isPublic(repository.url))
                    return bytes;
            }
            return await options.cache.read(repository.url, commit, path, maxBytes, signal);
        },
        async readme(dir, maxBytes, signal) {
            if (api !== undefined) {
                const read = (path, max, readSignal) => fromApi(path, max, readSignal, api);
                // The first name settles most lookups; the others are worth asking only where a miss is authoritative.
                const found = await readmeByName(read, dir, maxBytes, signal, README_NAMES.slice(0, 1));
                if (found !== undefined)
                    return found;
                if (await options.cache.isPublic(repository.url))
                    return await readmeByName(read, dir, maxBytes, signal, README_NAMES.slice(1));
            }
            return await options.cache.readme(repository.url, commit, dir, maxBytes, signal);
        },
    };
}
//# sourceMappingURL=detail-files.js.map