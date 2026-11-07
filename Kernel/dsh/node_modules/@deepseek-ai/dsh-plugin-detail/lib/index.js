import { lstat, readFile, readdir, realpath } from "node:fs/promises";
import { join, posix, relative, sep } from "node:path";
//#region lib/types/files.js
/**
* Read-only file sources for plugin detail documents: a local package directory and published npm package versions on
* public file CDNs, with bounded HTTP reads. Sources keep no state between calls.
*/
/** A read that must not fall back: the source exists but refused, failed, or exceeded a limit. */
var DetailFailure = class extends Error {
	reason;
	/**
	* @param reason - the failure kind a reader reports.
	* @param message - the diagnostic, naming the source.
	* @param options - the underlying error.
	*/
	constructor(reason, message, options) {
		super(message, options);
		this.reason = reason;
		this.name = "DetailFailure";
	}
};
/** README names tried in order where a source cannot list a directory. */
const README_NAMES = [
	"README.md",
	"readme.md",
	"Readme.md",
	"README.markdown",
	"README"
];
const README_PATTERN = /^readme(?:\.md|\.markdown)?$/iu;
/**
* Join a directory and a file name into a root-relative path.
* @param dir - directory relative to the root, `''` for the root.
* @param name - path relative to `dir`.
* @returns the joined path.
*/
function joinPath(dir, name) {
	return dir === "" ? name : `${dir}/${name}`;
}
/**
* Pick the README entry of a directory listing by the order of {@link README_NAMES}, then by name.
* @param names - file names of the directory.
* @returns the README name, or undefined.
*/
function readmeName(names) {
	const rank = (name) => {
		const index = README_NAMES.indexOf(name);
		return index === -1 ? README_NAMES.length : index;
	};
	return names.filter((name) => README_PATTERN.test(name)).sort((a, b) => rank(a) - rank(b) || (a < b ? -1 : 1))[0];
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
async function readmeByName(read, dir, maxBytes, signal, names = README_NAMES) {
	for (const name of names) {
		const path = joinPath(dir, name);
		const bytes = await read(path, maxBytes, signal);
		if (bytes !== void 0) return {
			path,
			bytes
		};
	}
}
function tooLarge(label, maxBytes) {
	return new DetailFailure("too-large", `${label} exceeds ${String(maxBytes)} bytes`);
}
/**
* Files of a local package directory. A path whose real location leaves the directory reads as absent.
* @param root - absolute package directory.
* @param revision - the installed version, so a reinstalled package reads as different files.
* @returns the files.
*/
function localFiles(root, revision) {
	const files = {
		key: `local:${root}@${revision}`,
		async read(path, maxBytes) {
			let file;
			try {
				const base = await realpath(root);
				file = await realpath(join(base, ...path.split("/")));
				const inside = relative(base, file);
				if (inside === "" || inside === ".." || inside.startsWith(`..${sep}`)) return void 0;
				const info = await lstat(file);
				if (!info.isFile()) return void 0;
				if (info.size > maxBytes) throw tooLarge(path, maxBytes);
			} catch (error) {
				if (error instanceof DetailFailure) throw error;
				return;
			}
			const bytes = await readFile(file);
			/* v8 ignore next -- the file grew between its size check and its read; no test controls that timing */
			if (bytes.byteLength > maxBytes) throw tooLarge(path, maxBytes);
			return bytes;
		},
		async readme(dir, maxBytes, signal) {
			let names;
			try {
				names = (await readdir(join(root, ...dir === "" ? [] : dir.split("/")), { withFileTypes: true })).filter((entry) => entry.isFile() || entry.isSymbolicLink()).map((entry) => entry.name);
			} catch (_missing) {
				return;
			}
			const name = readmeName(names);
			if (name === void 0) return void 0;
			const path = joinPath(dir, name);
			const bytes = await files.read(path, maxBytes, signal);
			return bytes === void 0 ? void 0 : {
				path,
				bytes
			};
		}
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
async function fetchBody(url, maxBytes, signal, options) {
	const combined = AbortSignal.any([
		signal,
		options.lifetime,
		AbortSignal.timeout(options.timeoutMs)
	]);
	let response;
	let target = url;
	for (let hop = 0; hop <= MAX_REDIRECTS; hop++) {
		try {
			response = await fetch(target, {
				signal: combined,
				redirect: "manual"
			});
		} catch (error) {
			signal.throwIfAborted();
			options.lifetime.throwIfAborted();
			throw new DetailFailure("network", `Could not fetch ${target}: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
		}
		const location = response.status >= 300 && response.status < 400 ? response.headers.get("location") : null;
		if (location === null || !URL.canParse(location, target) || new URL(location, target).origin !== new URL(url).origin) break;
		await response.body?.cancel();
		target = new URL(location, target).href;
	}
	const answer = response;
	if (!answer.ok) {
		await answer.body?.cancel();
		return {
			ok: false,
			status: answer.status
		};
	}
	if (Number(answer.headers.get("content-length") ?? 0) > maxBytes) {
		await answer.body?.cancel();
		throw tooLarge(url, maxBytes);
	}
	const chunks = [];
	let size = 0;
	try {
		for await (const chunk of answer.body ?? []) {
			size += chunk.byteLength;
			if (size > maxBytes) throw tooLarge(url, maxBytes);
			chunks.push(chunk);
		}
	} catch (error) {
		if (error instanceof DetailFailure) throw error;
		throw new DetailFailure("network", `Could not read ${url}: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
	}
	return {
		ok: true,
		bytes: Buffer.concat(chunks)
	};
}
/** Every npm file CDN, in the default order. */
const NPM_FILE_SOURCES = [
	"jsdelivr",
	"unpkg",
	"npmmirror"
];
/**
* Order the configured npm file CDNs by a caller's preference: the preferred ones that are configured, in the preferred
* order, then the other configured ones in configured order. A preferred CDN that is not configured is not asked.
* @param configured - the configured CDNs in order.
* @param preferred - CDNs to ask first, such as those a Client measured as fastest; names other than CDNs are ignored.
* @returns the CDNs to ask, first to last.
*/
function npmFileSourceOrder(configured, preferred = []) {
	const first = configured.filter((source) => preferred.includes(source)).sort((a, b) => preferred.indexOf(a) - preferred.indexOf(b));
	return [...first, ...configured.filter((source) => !first.includes(source))];
}
/**
* Percent-encode each segment of a `/`-separated path for a URL.
* @param path - the path.
* @returns the encoded path.
*/
function encodePath(path) {
	return path.split("/").map(encodeURIComponent).join("/");
}
const NPM_FILE_URLS = {
	jsdelivr: (name, version, path) => `https://cdn.jsdelivr.net/npm/${name}@${version}/${encodePath(path)}`,
	unpkg: (name, version, path) => `https://unpkg.com/${name}@${version}/${encodePath(path)}`,
	npmmirror: (name, version, path) => `https://registry.npmmirror.com/${name}/${version}/files/${encodePath(path)}`
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
function npmFiles(name, version, sources, options) {
	const files = {
		key: `npm:${name}@${version}`,
		async read(path, maxBytes, signal) {
			const failures = [];
			for (const source of sources) {
				const url = NPM_FILE_URLS[source](name, version, path);
				try {
					const body = await fetchBody(url, maxBytes, signal, options);
					if (body.ok) return body.bytes;
					if (body.status === 404) return void 0;
					failures.push(`${url}: HTTP ${String(body.status)}`);
				} catch (error) {
					if (!(error instanceof DetailFailure) || error.reason !== "network") throw error;
					failures.push(error.message);
				}
			}
			throw new DetailFailure("network", failures.join("; "));
		},
		readme: (dir, maxBytes, signal) => readmeByName((path, max, readSignal) => files.read(path, max, readSignal), dir, maxBytes, signal)
	};
	return files;
}
//#endregion
//#region lib/types/document.js
/**
* Plugin detail document selection: the Markdown file a package's locale `meta.detail` names in a display language,
* otherwise the package's README; and the images beside a selected document.
*/
const LANGUAGE_ID = /^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/u;
/**
* The languages a document is chosen in: the language, each shorter prefix, then English.
* @param language - a BCP 47 language id such as `zh-CN`.
* @returns lower-case language ids, most specific first.
* @throws when `language` is not a language id.
*/
function languageChain(language) {
	const id = language.toLowerCase();
	if (!LANGUAGE_ID.test(id)) throw new Error(`Not a language id: ${language}`);
	const parts = id.split("-");
	const chain = parts.map((_part, index) => parts.slice(0, parts.length - index).join("-"));
	return [...new Set([...chain, "en"])];
}
/**
* A locale `meta.detail` value: a `/`-separated Markdown path relative to the package root that cannot leave it.
* @param value - the declared `meta.detail` value, or `undefined` when the locale file omits it.
* @param label - the locale file path used in a failure message.
* @returns the validated path, or `undefined` when the value is absent.
* @throws {DetailFailure} `invalid` for another value.
*/
function detailPath(value, label) {
	if (value === void 0) return void 0;
	const invalid = (rule) => new DetailFailure("invalid", `${label} ${rule}`);
	if (typeof value !== "string" || value.trim() === "") throw invalid("must be a non-empty string");
	if (value.includes("\\") || /^[A-Za-z][A-Za-z\d+.-]*:/u.test(value) || value.split("/").some((segment) => segment === "" || segment === "." || segment === "..")) throw invalid("must be a relative path inside the package");
	if (!/\.(?:md|markdown)$/iu.test(value)) throw invalid("must name a Markdown file");
	return value;
}
/** `meta.detail` of the first `locale/<language>.json` along the chain that declares one. */
async function localeDetail(files, dir, languages, maxBytes, signal) {
	for (const language of languages) {
		const names = [...new Set([language, language.replace(/^([a-z]{2,3})-([a-z]{2})$/u, (_match, base, region) => `${base}-${region.toUpperCase()}`)])];
		for (const name of names) {
			const path = joinPath(dir, `locale/${name}.json`);
			const bytes = await files.read(path, maxBytes, signal);
			if (bytes === void 0) continue;
			let meta;
			try {
				meta = JSON.parse(new TextDecoder().decode(bytes))?.meta;
			} catch (error) {
				throw new DetailFailure("invalid", `${path}: ${error.message}`, { cause: error });
			}
			const detail = detailPath(typeof meta === "object" && meta !== null ? meta.detail : void 0, `${path}: meta.detail`);
			if (detail !== void 0) return detail;
			break;
		}
	}
}
/**
* Decode a Markdown document: UTF-8 without a byte order mark, without a leading YAML front matter block, which is file
* metadata rather than description text.
* @param bytes - the file's bytes.
* @returns the Markdown text.
*/
function markdownOf(bytes) {
	return new TextDecoder().decode(bytes).replace(/^\uFEFF/u, "").replace(/^---\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/u, "");
}
/**
* Read a package's detail document: the Markdown file that `meta.detail` in its `locale/<language>.json` names for the
* first language of the chain that declares one, otherwise its README.
* @param files - the package's files.
* @param dir - package directory inside `files`, `''` for its root.
* @param languages - the chain from {@link languageChain}.
* @param maxBytes - largest accepted locale file and document.
* @param signal - cancellation of the reads.
* @returns the document, or undefined when the package has neither.
* @throws {DetailFailure} `invalid` for a malformed locale file or `meta.detail` or a named file that does not exist, and
*   the failures of `files`.
*/
async function readPackageDocument(files, dir, languages, maxBytes, signal) {
	const detail = await localeDetail(files, dir, languages, maxBytes, signal);
	if (detail !== void 0) {
		const path = joinPath(dir, detail);
		const bytes = await files.read(path, maxBytes, signal);
		if (bytes === void 0) throw new DetailFailure("invalid", `${path}, named by meta.detail, does not exist`);
		return {
			origin: "locale",
			path,
			markdown: markdownOf(bytes)
		};
	}
	const readme = await files.readme(dir, maxBytes, signal);
	return readme === void 0 ? void 0 : {
		origin: "readme",
		path: readme.path,
		markdown: markdownOf(readme.bytes)
	};
}
/** Image file types served beside a document, by lower-case extension. */
const IMAGE_TYPES = new Map([
	[".png", "image/png"],
	[".jpg", "image/jpeg"],
	[".jpeg", "image/jpeg"],
	[".gif", "image/gif"],
	[".webp", "image/webp"],
	[".avif", "image/avif"],
	[".svg", "image/svg+xml"]
]);
/**
* Resolve an image destination a document writes.
* @param document - `/`-separated path of the document inside its files.
* @param authored - the destination as written: relative to the document, or to the root of its files when it starts
*   with `/`; a query or fragment is ignored.
* @returns the image's path inside the files and its media type, or undefined for a malformed destination, a path that
*   leaves the root, or a type other than PNG, JPEG, GIF, WebP, AVIF, and SVG.
*/
function detailImage(document, authored) {
	let decoded;
	try {
		decoded = decodeURIComponent(authored.replace(/[?#].*$/su, ""));
	} catch (_malformed) {
		return;
	}
	if (decoded.includes("\\") || decoded.includes("\0")) return void 0;
	const path = posix.normalize(decoded.startsWith("/") ? decoded.slice(1) : posix.join(posix.dirname(document), decoded));
	if (path === "." || path.startsWith("../") || path === ".." || posix.isAbsolute(path)) return void 0;
	const mediaType = IMAGE_TYPES.get(posix.extname(path).toLowerCase());
	return mediaType === void 0 ? void 0 : {
		path,
		mediaType
	};
}
//#endregion
//#region lib/types/response.js
/** HTTP status of each detail failure. */
const FAILURE_STATUS = {
	"too-large": 413,
	"network": 502,
	"denied": 403,
	"git-unavailable": 503,
	"invalid": 400
};
/**
* The response of an image route: the image, 404 when there is none, or the failure's status with its reason as the
* body. Every response carries `X-Content-Type-Options: nosniff` and a sandbox content security policy, because an SVG
* opened directly would otherwise run scripts on the authenticated API origin; only an image gets `cacheControl`.
* @param outcome - the image, undefined when none exists, or why it cannot be read; `invalid` also answers a malformed request.
* @param cacheControl - the `Cache-Control` value of an image.
* @returns the response.
*/
function detailImageResponse(outcome, cacheControl) {
	const [status, body, type] = typeof outcome === "object" ? [
		200,
		new Uint8Array(outcome.bytes),
		outcome.mediaType
	] : outcome === void 0 ? [
		404,
		"not found",
		void 0
	] : [
		FAILURE_STATUS[outcome],
		outcome,
		void 0
	];
	return new Response(body, {
		status,
		headers: {
			"Content-Type": type ?? "text/plain; charset=utf-8",
			"Cache-Control": status === 200 ? cacheControl : "no-store",
			"X-Content-Type-Options": "nosniff",
			"Content-Security-Policy": "sandbox; default-src 'none'; style-src 'unsafe-inline'"
		}
	});
}
//#endregion
export { DetailFailure, NPM_FILE_SOURCES, README_NAMES, detailImage, detailImageResponse, encodePath, fetchBody, joinPath, languageChain, localFiles, markdownOf, npmFileSourceOrder, npmFiles, readPackageDocument, readmeByName, readmeName };
