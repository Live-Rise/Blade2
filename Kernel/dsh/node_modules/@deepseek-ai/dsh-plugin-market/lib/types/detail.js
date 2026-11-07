/**
 * Plugin market entry detail documents: the package document {@link readPackageDocument} selects from the entry
 * version's source, otherwise the README of the repository directory the entry or its package declares; cached reads
 * and the images beside a read document.
 */
import { createHash } from 'node:crypto';
import { posix } from 'node:path';
import { brandString } from '@deepseek-ai/dsh-brand';
import { DetailFailure, detailImage, joinPath, markdownOf, readPackageDocument } from '@deepseek-ai/dsh-plugin-detail';
import hostedGitInfo from 'hosted-git-info';
import { repositoryFiles } from "./detail-files.js";
/** Failures that reading again within `ttlMs` would repeat. */
const LASTING_FAILURES = new Set(['too-large', 'denied', 'invalid']);
/**
 * The repository a URL or shorthand names: hosts npm knows by their owner and project, so a GitHub page such as
 * `/o/r/tree/main/pkg` names `o/r`; other http(s) URLs by their path, without a GitLab `/-/` page suffix.
 * @param url - a manifest `repository` URL or shorthand, or a market index's `repository.url`.
 * @returns the repository, or undefined for a gist, another protocol, or fewer than two path segments.
 */
export function repositoryOf(url) {
    // A committish such as `#semver:^1` names a revision, not the repository.
    const declared = url.replace(/#.*$/u, '');
    const hosted = hostedGitInfo.fromUrl(declared);
    if (hosted !== undefined) {
        if (hosted.type === 'gist')
            return undefined;
        const project = `${hosted.user}/${hosted.project}`;
        return { host: hosted.domain.toLowerCase(), project, url: `https://${hosted.domain}/${project}.git` };
    }
    const address = declared.replace(/^git\+/u, '');
    if (!URL.canParse(address))
        return undefined;
    const parsed = new URL(address);
    const project = parsed.pathname.replace(/\/-\/.*$/u, '').replace(/^\/+|\/+$/gu, '').replace(/\.git$/u, '');
    if ((parsed.protocol !== 'https:' && parsed.protocol !== 'http:') || parsed.username !== '' || parsed.password !== '' || project.split('/').length < 2) {
        return undefined;
    }
    return { host: parsed.hostname.toLowerCase(), project, url: `${parsed.protocol}//${parsed.host}/${project}.git` };
}
/**
 * Parse a manifest `repository` into the repository and directory its README is read from.
 * @param value - a manifest `repository` string or object.
 * @returns the location, or undefined when it names no repository {@link repositoryOf} accepts; a `directory` that
 *   leaves the repository is dropped.
 */
export function manifestRepository(value) {
    const object = typeof value === 'object' && value !== null ? value : undefined;
    const declared = object === undefined ? value : object.url;
    if (typeof declared !== 'string')
        return undefined;
    const repository = repositoryOf(declared);
    if (repository === undefined)
        return undefined;
    return { ...repository, directory: typeof object?.directory === 'string' ? normalizedDirectory(object.directory) ?? '' : '' };
}
/** A `/`-separated directory without empty, `.`, or `..` segments; undefined for one that leaves its root. */
function normalizedDirectory(directory) {
    const normalized = posix.normalize(directory.replace(/\\/gu, '/')).replace(/\/+$/u, '');
    if (normalized === '.' || normalized === '')
        return '';
    return normalized === '..' || normalized.startsWith('../') || posix.isAbsolute(normalized) ? undefined : normalized;
}
/** Reads and caches detail documents for one service. */
export class DetailReader {
    limits;
    repositories;
    results = new Map();
    pending = new Map();
    documents = new Map();
    /**
     * @param limits - document and image sizes and the result lifetime.
     * @param repositories - single-file API, clone, and request settings of repository reads.
     */
    constructor(limits, repositories) {
        this.limits = limits;
        this.repositories = repositories;
    }
    /**
     * Read a package's detail document, sharing concurrent reads and reusing a result, or a failure that reading again
     * would repeat, for `ttlMs`.
     * @param location - the package's sources.
     * @param languages - the chain from `languageChain` of `dsh-plugin-detail`.
     * @param refresh - read again instead of reusing a cached result.
     * @returns the first document the sources provide, or `none`.
     * @throws {DetailFailure} when a source that should provide a document cannot be read.
     */
    read(location, languages, refresh = false) {
        const { repository } = location;
        const repositoryKey = typeof repository === 'string' ? repository : repository === undefined ? '' : `${repository.url}:${repository.directory}`;
        const key = JSON.stringify([location.files?.key ?? '', location.dir, repositoryKey, languages]);
        const now = Date.now();
        const cached = this.results.get(key);
        if (!refresh && cached !== undefined && cached.expiresAt > now) {
            if ('failure' in cached)
                return Promise.reject(cached.failure);
            if (cached.value.status === 'found')
                this.use(cached.value.document);
            return Promise.resolve(cached.value);
        }
        let pending = this.pending.get(key);
        if (pending === undefined) {
            pending = this.resolve(location, languages, this.repositories.lifetime).then((value) => {
                this.remember(key, { value, expiresAt: Date.now() + this.limits.ttlMs });
                return value;
            }, (error) => {
                if (error instanceof DetailFailure && LASTING_FAILURES.has(error.reason)) {
                    this.remember(key, { failure: error, expiresAt: Date.now() + this.limits.ttlMs });
                }
                throw error;
            }).finally(() => { this.pending.delete(key); });
            this.pending.set(key, pending);
        }
        return pending;
    }
    /**
     * Read an image beside a document.
     * @param id - the document id a detail read returned.
     * @param authored - the image destination as the document writes it: relative to the document, or to the package or
     *   repository root when it starts with `/`; a query or fragment is ignored.
     * @returns the bytes and media type, or undefined for an unknown document, a path outside its root, an unsupported
     *   image type, or a missing file.
     * @throws {DetailFailure} when the source fails or the image exceeds `maxImageBytes`.
     */
    async image(id, authored) {
        const document = this.use(brandString(id));
        const image = document === undefined ? undefined : detailImage(document.path, authored);
        if (document === undefined || image === undefined)
            return undefined;
        // A request the browser abandons still completes, so a cancelled image never interrupts a shared clone.
        const bytes = await document.files.read(image.path, this.limits.maxImageBytes, this.repositories.lifetime);
        return bytes === undefined ? undefined : { bytes, mediaType: image.mediaType };
    }
    /**
     * Mark a document used and forget documents unused for `ttlMs`, so ids live as long as a read or an image request
     * returns them. A cached read is younger than `ttlMs` and marks its document, so no cached read names a forgotten id.
     */
    use(id) {
        const now = Date.now();
        for (const [known, document] of this.documents)
            if (now - document.usedAt > this.limits.ttlMs)
                this.documents.delete(known);
        const document = this.documents.get(id);
        if (document !== undefined)
            document.usedAt = now;
        return document;
    }
    /** Cache a read and drop expired ones, so the cache holds only reads younger than `ttlMs`. */
    remember(key, read) {
        const now = Date.now();
        for (const [cachedKey, cached] of this.results)
            if (cached.expiresAt <= now)
                this.results.delete(cachedKey);
        this.results.set(key, read);
    }
    async resolve(location, languages, signal) {
        const { files, dir } = location;
        let repository = location.repository === 'manifest' ? undefined : location.repository;
        if (files !== undefined) {
            const document = await readPackageDocument(files, dir, languages, this.limits.maxDocumentBytes, signal);
            if (document !== undefined)
                return this.found(document.origin, files, document.path, document.markdown);
            if (location.repository === 'manifest')
                repository = await this.manifestRepositoryOf(files, dir, signal);
        }
        if (repository === undefined)
            return { status: 'none' };
        const source = repositoryFiles(repository, undefined, this.repositories);
        const readme = await source.readme(repository.directory, this.limits.maxDocumentBytes, signal);
        return readme === undefined ? { status: 'none' } : this.found('repository', source, readme.path, markdownOf(readme.bytes));
    }
    async manifestRepositoryOf(files, dir, signal) {
        const bytes = await files.read(joinPath(dir, 'package.json'), this.limits.maxDocumentBytes, signal);
        if (bytes === undefined)
            return undefined;
        try {
            return manifestRepository(JSON.parse(new TextDecoder().decode(bytes))?.repository);
        }
        catch (_malformed) {
            // A manifest the package manager accepted parses; an unreadable one names no repository to read.
            return undefined;
        }
    }
    found(origin, files, path, markdown) {
        const document = brandString(createHash('sha256').update(`${files.key}\0${path}`).digest('hex').slice(0, 32));
        this.documents.set(document, { files, path, usedAt: Date.now() });
        this.use(document);
        return { status: 'found', origin, markdown, document };
    }
}
//# sourceMappingURL=detail.js.map