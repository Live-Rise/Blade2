/**
 * Read-only file sources for plugin detail documents: a local package directory and published npm package versions on
 * public file CDNs, with bounded HTTP reads. Sources keep no state between calls.
 */
import { lstat, readdir, readFile, realpath } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';
/** A read that must not fall back: the source exists but refused, failed, or exceeded a limit. */
export class DetailFailure extends Error {
    reason;
    /**
     * @param reason - the failure kind a reader reports.
     * @param message - the diagnostic, naming the source.
     * @param options - the underlying error.
     */
    constructor(reason, message, options) {
        super(message, options);
        this.reason = reason;
        this.name = 'DetailFailure';
    }
}
/** README names tried in order where a source cannot list a directory. */
export const README_NAMES = ['README.md', 'readme.md', 'Readme.md', 'README.markdown', 'README'];
const README_PATTERN = /^readme(?:\.md|\.markdown)?$/iu;
/**
 * Join a directory and a file name into a root-relative path.
 * @param dir - directory relative to the root, `''` for the root.
 * @param name - path relative to `dir`.
 * @returns the joined path.
 */
export function joinPath(dir, name) {
    return dir === '' ? name : `${dir}/${name}`;
}
/**
 * Pick the README entry of a directory listing by the order of {@link README_NAMES}, then by name.
 * @param names - file names of the directory.
 * @returns the README name, or undefined.
 */
export function readmeName(names) {
    const rank = (name) => {
        const index = README_NAMES.indexOf(name);
        return index === -1 ? README_NAMES.length : index;
    };
    return names.filter(name => README_PATTERN.test(name)).sort((a, b) => rank(a) - rank(b) || (a < b ? -1 : 1))[0];
}
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
export async function readmeByName(read, dir, maxBytes, signal, names = README_NAMES) {
    for (const name of names) {
        const path = joinPath(dir, name);
        const bytes = await read(path, maxBytes, signal);
        if (bytes !== undefined)
            return { path, bytes };
    }
    return undefined;
}
function tooLarge(label, maxBytes) {
    return new DetailFailure('too-large', `${label} exceeds ${String(maxBytes)} bytes`);
}
/**
 * Files of a local package directory. A path whose real location leaves the directory reads as absent.
 * @param root - absolute package directory.
 * @param revision - the installed version, so a reinstalled package reads as different files.
 * @returns the files.
 */
export function localFiles(root, revision) {
    const files = {
        key: `local:${root}@${revision}`,
        async read(path, maxBytes) {
            let file;
            try {
                const base = await realpath(root);
                file = await realpath(join(base, ...path.split('/')));
                const inside = relative(base, file);
                if (inside === '' || inside === '..' || inside.startsWith(`..${sep}`))
                    return undefined;
                const info = await lstat(file);
                if (!info.isFile())
                    return undefined;
                if (info.size > maxBytes)
                    throw tooLarge(path, maxBytes);
            }
            catch (error) {
                if (error instanceof DetailFailure)
                    throw error;
                // A missing file or directory, or one the process may not read, is a file the package does not provide.
                return undefined;
            }
            const bytes = await readFile(file);
            /* v8 ignore next -- the file grew between its size check and its read; no test controls that timing */
            if (bytes.byteLength > maxBytes)
                throw tooLarge(path, maxBytes);
            return bytes;
        },
        async readme(dir, maxBytes, signal) {
            let names;
            try {
                names = (await readdir(join(root, ...dir === '' ? [] : dir.split('/')), { withFileTypes: true }))
                    .filter(entry => entry.isFile() || entry.isSymbolicLink()).map(entry => entry.name);
            }
            catch (_missing) {
                // An absent or unreadable directory has no README.
                return undefined;
            }
            const name = readmeName(names);
            if (name === undefined)
                return undefined;
            const path = joinPath(dir, name);
            const bytes = await files.read(path, maxBytes, signal);
            return bytes === undefined ? undefined : { path, bytes };
        },
    };
    return files;
}
/** Redirects followed within one origin, such as a renamed repository's raw files; a redirect to another origin is not followed. */
const MAX_REDIRECTS = 3;
/**
 * GET a URL, following redirects within its origin, and read the body up to `maxBytes`.
 * @param url - the request URL.
 * @param maxBytes - largest accepted body.
 * @param signal - cancellation of the read.
 * @param options - deadline and service lifetime.
 * @returns the body, or the status of a non-2xx response, including a redirect to another origin.
 * @throws {DetailFailure} `network` when no response arrives or the body fails, `too-large` over `maxBytes`.
 */
export async function fetchBody(url, maxBytes, signal, options) {
    const combined = AbortSignal.any([signal, options.lifetime, AbortSignal.timeout(options.timeoutMs)]);
    let response;
    let target = url;
    for (let hop = 0; hop <= MAX_REDIRECTS; hop++) {
        try {
            response = await fetch(target, { signal: combined, redirect: 'manual' });
        }
        catch (error) {
            // A cancelled read says nothing about the source, so it keeps the caller's reason.
            signal.throwIfAborted();
            options.lifetime.throwIfAborted();
            throw new DetailFailure('network', `Could not fetch ${target}: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
        }
        const location = response.status >= 300 && response.status < 400 ? response.headers.get('location') : null;
        if (location === null || !URL.canParse(location, target) || new URL(location, target).origin !== new URL(url).origin)
            break;
        await response.body?.cancel();
        target = new URL(location, target).href;
    }
    // The loop runs at least once.
    const answer = response;
    if (!answer.ok) {
        await answer.body?.cancel();
        return { ok: false, status: answer.status };
    }
    if (Number(answer.headers.get('content-length') ?? 0) > maxBytes) {
        await answer.body?.cancel();
        throw tooLarge(url, maxBytes);
    }
    const chunks = [];
    let size = 0;
    try {
        for await (const chunk of answer.body ?? []) {
            size += chunk.byteLength;
            if (size > maxBytes)
                throw tooLarge(url, maxBytes);
            chunks.push(chunk);
        }
    }
    catch (error) {
        if (error instanceof DetailFailure)
            throw error;
        throw new DetailFailure('network', `Could not read ${url}: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
    }
    return { ok: true, bytes: Buffer.concat(chunks) };
}
/** Every npm file CDN, in the default order. */
export const NPM_FILE_SOURCES = ['jsdelivr', 'unpkg', 'npmmirror'];
/**
 * Order the configured npm file CDNs by a caller's preference: the preferred ones that are configured, in the preferred
 * order, then the other configured ones in configured order. A preferred CDN that is not configured is not asked.
 * @param configured - the configured CDNs in order.
 * @param preferred - CDNs to ask first, such as those a Client measured as fastest; names other than CDNs are ignored.
 * @returns the CDNs to ask, first to last.
 */
export function npmFileSourceOrder(configured, preferred = []) {
    const first = configured.filter(source => preferred.includes(source)).sort((a, b) => preferred.indexOf(a) - preferred.indexOf(b));
    return [...first, ...configured.filter(source => !first.includes(source))];
}
/**
 * Percent-encode each segment of a `/`-separated path for a URL.
 * @param path - the path.
 * @returns the encoded path.
 */
export function encodePath(path) {
    return path.split('/').map(encodeURIComponent).join('/');
}
const NPM_FILE_URLS = {
    jsdelivr: (name, version, path) => `https://cdn.jsdelivr.net/npm/${name}@${version}/${encodePath(path)}`,
    unpkg: (name, version, path) => `https://unpkg.com/${name}@${version}/${encodePath(path)}`,
    npmmirror: (name, version, path) => `https://registry.npmmirror.com/${name}/${version}/files/${encodePath(path)}`,
};
/**
 * Files of one published npm package version, read from public file CDNs in order. A 404 from a CDN means the package
 * does not contain the file; a network failure or another status asks the next CDN.
 * @param name - package name.
 * @param version - exact version.
 * @param sources - the CDNs to ask, first to last; never empty.
 * @param options - deadline and service lifetime.
 * @returns the files.
 */
export function npmFiles(name, version, sources, options) {
    const files = {
        key: `npm:${name}@${version}`,
        async read(path, maxBytes, signal) {
            const failures = [];
            for (const source of sources) {
                const url = NPM_FILE_URLS[source](name, version, path);
                try {
                    const body = await fetchBody(url, maxBytes, signal, options);
                    if (body.ok)
                        return body.bytes;
                    if (body.status === 404)
                        return undefined;
                    failures.push(`${url}: HTTP ${String(body.status)}`);
                }
                catch (error) {
                    if (!(error instanceof DetailFailure) || error.reason !== 'network')
                        throw error;
                    failures.push(error.message);
                }
            }
            throw new DetailFailure('network', failures.join('; '));
        },
        readme: (dir, maxBytes, signal) => readmeByName((path, max, readSignal) => files.read(path, max, readSignal), dir, maxBytes, signal),
    };
    return files;
}
//# sourceMappingURL=files.js.map