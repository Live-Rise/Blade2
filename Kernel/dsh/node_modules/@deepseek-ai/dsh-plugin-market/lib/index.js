import { isAbsolute, join, posix, resolve } from "node:path";
import z from "@deepseek-ai/schemastery";
import semver from "semver";
import { evaluatePluginCompatibility, getDshRuntimeVersion } from "@deepseek-ai/dsh-app-boot";
import { marketInstallTarget } from "@deepseek-ai/dsh-plugin-market-index";
import { MAX_TIMER_DELAY_MS } from "@deepseek-ai/dsh-timeout";
import { Remote, RemoteError, TypertRemoteService } from "@deepseek-ai/dsh-typert-protocol";
import { DetailFailure, NPM_FILE_SOURCES, README_NAMES, detailImage, detailImageResponse, encodePath, fetchBody, joinPath, languageChain, markdownOf, npmFileSourceOrder, npmFiles, readPackageDocument, readmeByName, readmeName } from "@deepseek-ai/dsh-plugin-detail";
import { createHash } from "node:crypto";
import { brandString } from "@deepseek-ai/dsh-brand";
import hostedGitInfo from "hosted-git-info";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readdir, rename, rm, stat, utimes } from "node:fs/promises";
import { execa } from "execa";
import { withFileLock } from "@deepseek-ai/dsh-atomic-write";
import { scrubbedParentEnv } from "@deepseek-ai/dsh-subprocess";
import { Worker } from "node:worker_threads";
import MiniSearch from "minisearch";
import { assertNever } from "@deepseek-ai/dsh-util-values";
//#region lib/types/detail-files.js
/**
* Git repository files for plugin detail documents, through GitHub or GitLab single-file APIs with a cached partial
* clone as the fallback for repositories those APIs cannot serve anonymously.
*/
function apiUrl(api, repository, ref, path) {
	return api === "github" ? `https://raw.githubusercontent.com/${repository.project}/${encodeURIComponent(ref)}/${encodePath(path)}` : `${new URL(repository.url).origin}/api/v4/projects/${encodeURIComponent(repository.project)}/repository/files/${encodeURIComponent(path)}/raw?ref=${encodeURIComponent(ref)}`;
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
function repositoryFiles(repository, commit, options) {
	const api = repository.host === "github.com" ? "github" : options.gitlabHosts.includes(repository.host) ? "gitlab" : void 0;
	const ref = commit ?? "HEAD";
	/** An API read: the bytes, or undefined for an answer that does not distinguish a missing file from a private repository. */
	const fromApi = async (path, maxBytes, signal, via) => {
		const body = await fetchBody(apiUrl(via, repository, ref, path), maxBytes, signal, options);
		if (body.ok) return body.bytes;
		if (body.status !== 404 && body.status !== 401 && body.status !== 403) throw new DetailFailure("network", `${repository.url}: ${path} answered HTTP ${String(body.status)}`);
	};
	return {
		key: `git:${repository.url}#${ref}`,
		async read(path, maxBytes, signal) {
			if (api !== void 0) {
				const bytes = await fromApi(path, maxBytes, signal, api);
				if (bytes !== void 0 || await options.cache.isPublic(repository.url)) return bytes;
			}
			return await options.cache.read(repository.url, commit, path, maxBytes, signal);
		},
		async readme(dir, maxBytes, signal) {
			if (api !== void 0) {
				const read = (path, max, readSignal) => fromApi(path, max, readSignal, api);
				const found = await readmeByName(read, dir, maxBytes, signal, README_NAMES.slice(0, 1));
				if (found !== void 0) return found;
				if (await options.cache.isPublic(repository.url)) return await readmeByName(read, dir, maxBytes, signal, README_NAMES.slice(1));
			}
			return await options.cache.readme(repository.url, commit, dir, maxBytes, signal);
		}
	};
}
//#endregion
//#region lib/types/detail.js
/**
* Plugin market entry detail documents: the package document {@link readPackageDocument} selects from the entry
* version's source, otherwise the README of the repository directory the entry or its package declares; cached reads
* and the images beside a read document.
*/
/** Failures that reading again within `ttlMs` would repeat. */
const LASTING_FAILURES = new Set([
	"too-large",
	"denied",
	"invalid"
]);
/**
* The repository a URL or shorthand names: hosts npm knows by their owner and project, so a GitHub page such as
* `/o/r/tree/main/pkg` names `o/r`; other http(s) URLs by their path, without a GitLab `/-/` page suffix.
* @param url - a manifest `repository` URL or shorthand, or a market index's `repository.url`.
* @returns the repository, or undefined for a gist, another protocol, or fewer than two path segments.
*/
function repositoryOf(url) {
	const declared = url.replace(/#.*$/u, "");
	const hosted = hostedGitInfo.fromUrl(declared);
	if (hosted !== void 0) {
		if (hosted.type === "gist") return void 0;
		const project = `${hosted.user}/${hosted.project}`;
		return {
			host: hosted.domain.toLowerCase(),
			project,
			url: `https://${hosted.domain}/${project}.git`
		};
	}
	const address = declared.replace(/^git\+/u, "");
	if (!URL.canParse(address)) return void 0;
	const parsed = new URL(address);
	const project = parsed.pathname.replace(/\/-\/.*$/u, "").replace(/^\/+|\/+$/gu, "").replace(/\.git$/u, "");
	if (parsed.protocol !== "https:" && parsed.protocol !== "http:" || parsed.username !== "" || parsed.password !== "" || project.split("/").length < 2) return;
	return {
		host: parsed.hostname.toLowerCase(),
		project,
		url: `${parsed.protocol}//${parsed.host}/${project}.git`
	};
}
/**
* Parse a manifest `repository` into the repository and directory its README is read from.
* @param value - a manifest `repository` string or object.
* @returns the location, or undefined when it names no repository {@link repositoryOf} accepts; a `directory` that
*   leaves the repository is dropped.
*/
function manifestRepository(value) {
	const object = typeof value === "object" && value !== null ? value : void 0;
	const declared = object === void 0 ? value : object.url;
	if (typeof declared !== "string") return void 0;
	const repository = repositoryOf(declared);
	if (repository === void 0) return void 0;
	return {
		...repository,
		directory: typeof object?.directory === "string" ? normalizedDirectory(object.directory) ?? "" : ""
	};
}
/** A `/`-separated directory without empty, `.`, or `..` segments; undefined for one that leaves its root. */
function normalizedDirectory(directory) {
	const normalized = posix.normalize(directory.replace(/\\/gu, "/")).replace(/\/+$/u, "");
	if (normalized === "." || normalized === "") return "";
	return normalized === ".." || normalized.startsWith("../") || posix.isAbsolute(normalized) ? void 0 : normalized;
}
/** Reads and caches detail documents for one service. */
var DetailReader = class {
	limits;
	repositories;
	results = /* @__PURE__ */ new Map();
	pending = /* @__PURE__ */ new Map();
	documents = /* @__PURE__ */ new Map();
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
		const repositoryKey = typeof repository === "string" ? repository : repository === void 0 ? "" : `${repository.url}:${repository.directory}`;
		const key = JSON.stringify([
			location.files?.key ?? "",
			location.dir,
			repositoryKey,
			languages
		]);
		const now = Date.now();
		const cached = this.results.get(key);
		if (!refresh && cached !== void 0 && cached.expiresAt > now) {
			if ("failure" in cached) return Promise.reject(cached.failure);
			if (cached.value.status === "found") this.use(cached.value.document);
			return Promise.resolve(cached.value);
		}
		let pending = this.pending.get(key);
		if (pending === void 0) {
			pending = this.resolve(location, languages, this.repositories.lifetime).then((value) => {
				this.remember(key, {
					value,
					expiresAt: Date.now() + this.limits.ttlMs
				});
				return value;
			}, (error) => {
				if (error instanceof DetailFailure && LASTING_FAILURES.has(error.reason)) this.remember(key, {
					failure: error,
					expiresAt: Date.now() + this.limits.ttlMs
				});
				throw error;
			}).finally(() => {
				this.pending.delete(key);
			});
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
		const image = document === void 0 ? void 0 : detailImage(document.path, authored);
		if (document === void 0 || image === void 0) return void 0;
		const bytes = await document.files.read(image.path, this.limits.maxImageBytes, this.repositories.lifetime);
		return bytes === void 0 ? void 0 : {
			bytes,
			mediaType: image.mediaType
		};
	}
	/**
	* Mark a document used and forget documents unused for `ttlMs`, so ids live as long as a read or an image request
	* returns them. A cached read is younger than `ttlMs` and marks its document, so no cached read names a forgotten id.
	*/
	use(id) {
		const now = Date.now();
		for (const [known, document] of this.documents) if (now - document.usedAt > this.limits.ttlMs) this.documents.delete(known);
		const document = this.documents.get(id);
		if (document !== void 0) document.usedAt = now;
		return document;
	}
	/** Cache a read and drop expired ones, so the cache holds only reads younger than `ttlMs`. */
	remember(key, read) {
		const now = Date.now();
		for (const [cachedKey, cached] of this.results) if (cached.expiresAt <= now) this.results.delete(cachedKey);
		this.results.set(key, read);
	}
	async resolve(location, languages, signal) {
		const { files, dir } = location;
		let repository = location.repository === "manifest" ? void 0 : location.repository;
		if (files !== void 0) {
			const document = await readPackageDocument(files, dir, languages, this.limits.maxDocumentBytes, signal);
			if (document !== void 0) return this.found(document.origin, files, document.path, document.markdown);
			if (location.repository === "manifest") repository = await this.manifestRepositoryOf(files, dir, signal);
		}
		if (repository === void 0) return { status: "none" };
		const source = repositoryFiles(repository, void 0, this.repositories);
		const readme = await source.readme(repository.directory, this.limits.maxDocumentBytes, signal);
		return readme === void 0 ? { status: "none" } : this.found("repository", source, readme.path, markdownOf(readme.bytes));
	}
	async manifestRepositoryOf(files, dir, signal) {
		const bytes = await files.read(joinPath(dir, "package.json"), this.limits.maxDocumentBytes, signal);
		if (bytes === void 0) return void 0;
		try {
			return manifestRepository(JSON.parse(new TextDecoder().decode(bytes))?.repository);
		} catch (_malformed) {
			return;
		}
	}
	found(origin, files, path, markdown) {
		const document = brandString(createHash("sha256").update(`${files.key}\0${path}`).digest("hex").slice(0, 32));
		this.documents.set(document, {
			files,
			path,
			usedAt: Date.now()
		});
		this.use(document);
		return {
			status: "found",
			origin,
			markdown,
			document
		};
	}
};
//#endregion
//#region lib/types/index-url.js
/** Index source URL validation shared by `setIndexes` and the per-use check of configured sources. */
/**
* Validate and normalize one index source URL.
* @param text - URL as configured or entered.
* @returns the URL's serialization.
* @throws {RemoteError} `plugin-market/invalid-index-url` for anything but an http(s) URL without credentials or
* fragment.
*/
function normalizeIndexUrl(text) {
	let url;
	try {
		url = new URL(text.trim());
	} catch {
		throw new RemoteError("plugin-market/invalid-index-url", `Not an absolute URL: ${text}`, {
			url: text,
			reason: "not-absolute"
		});
	}
	if (url.protocol !== "https:" && url.protocol !== "http:") throw new RemoteError("plugin-market/invalid-index-url", `Index URLs use http or https: ${text}`, {
		url: text,
		reason: "scheme"
	});
	if (url.username !== "" || url.password !== "" || url.href.includes("#")) throw new RemoteError("plugin-market/invalid-index-url", `Index URLs carry no credentials or fragment: ${text}`, {
		url: text,
		reason: "credentials-or-fragment"
	});
	return url.href;
}
//#endregion
//#region lib/types/repository-cache.js
/**
* Cached partial clones for plugin detail documents of repositories that need the user's Git credentials or have no
* anonymous single-file API. Each repository is a bare repository fetched with depth 1 and without file contents; Git
* downloads a file's contents only when it is read. No working tree is created, so no checkout filter, hook, LFS
* download, or symbolic link takes effect. A repository whose operation fails or is cancelled is deleted, so an
* interrupted command never leaves a damaged clone behind.
*/
/**
* Git deadlines one operation's lock waits for: an operation runs at most a fetch and a file read that contact the
* remote, each bounded by the deadline, plus short local commands.
*/
const OPERATION_COMMANDS = 4;
/** Output that names a connection failure or a timeout rather than a refusal by the server. */
const NETWORK_FAILURE = new RegExp([
	"Could not resolve host",
	"Failed to connect",
	"Connection (?:refused|reset)",
	"timed out",
	"Network is unreachable",
	"SSL",
	"TLS"
].join("|"), "iu");
/** A file over the size its read accepts; the clone itself stays usable. */
var FileTooLarge = class extends DetailFailure {
	/** @param path - the file. @param maxBytes - the accepted size. */
	constructor(path, maxBytes) {
		super("too-large", `${path} exceeds ${String(maxBytes)} bytes`);
	}
};
/** The failure of a Git command that contacts the remote: `network` for a timeout or a connection failure, otherwise `denied`. */
function remoteFailureOf(result, label) {
	return new DetailFailure(NETWORK_FAILURE.test(result.diagnostic) ? "network" : "denied", `${label}: ${result.diagnostic}`);
}
async function sizeOf(path) {
	let total = 0;
	for (const entry of await readdir(path, {
		withFileTypes: true,
		recursive: true
	})) if (entry.isFile()) total += (await stat(join(entry.parentPath, entry.name))).size;
	return total;
}
/** Staging directories that an initialization in progress owns; an older one was left by a stopped process. */
const STAGING_PREFIX = ".init-";
/**
* The environment of every Git command: the scrubbed parent environment without Git overrides such as `GIT_DIR`, which a
* hook that launched DSH can leave, and without prompts or LFS downloads.
*/
function gitEnv() {
	const env = Object.fromEntries(Object.entries(scrubbedParentEnv()).filter(([name]) => !name.startsWith("GIT_") || name === "GIT_SSH" || name === "GIT_SSH_COMMAND"));
	const ssh = env.GIT_SSH === void 0 && env.GIT_SSH_COMMAND === void 0 ? { GIT_SSH_COMMAND: "ssh -o BatchMode=yes" } : {};
	return {
		...env,
		...ssh,
		LC_ALL: "C",
		GIT_TERMINAL_PROMPT: "0",
		GIT_ASKPASS: "",
		SSH_ASKPASS: "",
		SSH_ASKPASS_REQUIRE: "never",
		GCM_INTERACTIVE: "never",
		GIT_LFS_SKIP_SMUDGE: "1",
		GIT_LITERAL_PATHSPECS: "1"
	};
}
/** Partial clones of detail repositories under one directory, owned by one service. */
var RepositoryCache = class {
	options;
	locks = /* @__PURE__ */ new Map();
	heads = /* @__PURE__ */ new Map();
	/**
	* Whether `git --version` has succeeded. A missing Git is checked this way before any operation, because Windows
	* reports a missing command as an ordinary exit code that would otherwise read as a refused fetch.
	*/
	gitStarts = false;
	visibility = /* @__PURE__ */ new Map();
	checks = /* @__PURE__ */ new Map();
	running = /* @__PURE__ */ new Set();
	/** @param options - location, limits, deadline, transports, and lifetime. */
	constructor(options) {
		this.options = options;
	}
	/**
	* Whether a repository's smart HTTP endpoint answers without credentials, which makes a missing single-file API answer
	* authoritative. The request carries no user configuration, credential helper, or `.netrc` entry; concurrent checks of
	* one repository share it, and its answer is reused for `ttlMs`.
	* @param url - the repository's https URL, ending in `.git`.
	* @returns true for a 2xx answer; false for 401, 403, 404, or a redirect, which hosts send for a private repository.
	* @throws {DetailFailure} `network` when the host cannot be reached or answers another status.
	*/
	isPublic(url) {
		const cached = this.visibility.get(url);
		if (cached !== void 0 && cached.expiresAt > Date.now()) return Promise.resolve(cached.value);
		let pending = this.checks.get(url);
		if (pending === void 0) {
			pending = this.check(url).finally(() => {
				this.checks.delete(url);
			});
			this.checks.set(url, pending);
		}
		return pending;
	}
	async check(url) {
		const endpoint = `${url}/info/refs?service=git-upload-pack`;
		let response;
		try {
			response = await fetch(endpoint, {
				redirect: "manual",
				signal: AbortSignal.any([this.options.lifetime, AbortSignal.timeout(this.options.timeoutMs)])
			});
		} catch (error) {
			throw new DetailFailure("network", `Could not reach ${endpoint}: ${error.message}`, { cause: error });
		}
		await response.body?.cancel();
		const { status } = response;
		if (!response.ok && status !== 401 && status !== 403 && status !== 404 && (status < 300 || status > 399)) throw new DetailFailure("network", `${endpoint} answered HTTP ${String(status)}`);
		this.visibility.set(url, {
			value: response.ok,
			expiresAt: Date.now() + this.options.ttlMs
		});
		return response.ok;
	}
	/**
	* Read one regular file of a repository.
	* @param url - the repository URL.
	* @param commit - a full commit id, or undefined for the default branch.
	* @param path - `/`-separated path relative to the repository root.
	* @param maxBytes - largest accepted size.
	* @param signal - cancellation of the read.
	* @returns the bytes, or undefined when the commit has no regular file at `path`.
	* @throws {DetailFailure} when Git cannot fetch the commit, the repository grows past its limit, or the file exceeds `maxBytes`.
	*/
	read(url, commit, path, maxBytes, signal) {
		return this.locked(url, async (repository) => {
			const resolved = await this.commit(repository, url, commit, signal);
			const blob = (await this.files(repository, resolved, path, signal)).get(path);
			return blob === void 0 ? void 0 : await this.blob(repository, url, blob, path, maxBytes, signal);
		});
	}
	/**
	* Read the README of a repository directory: `README.md`, `README.markdown`, or `README` in any letter case.
	* @param url - the repository URL.
	* @param commit - a full commit id, or undefined for the default branch.
	* @param dir - directory relative to the repository root; `''` for the root.
	* @param maxBytes - largest accepted size.
	* @param signal - cancellation of the read.
	* @returns the file, or undefined when the directory has none.
	* @throws {DetailFailure} as {@link read} does.
	*/
	readme(url, commit, dir, maxBytes, signal) {
		return this.locked(url, async (repository) => {
			const resolved = await this.commit(repository, url, commit, signal);
			const files = await this.files(repository, resolved, dir === "" ? void 0 : `${dir}/`, signal);
			const prefix = dir === "" ? "" : `${dir}/`;
			const name = readmeName([...files.keys()].map((path) => path.slice(prefix.length)));
			if (name === void 0) return void 0;
			const path = `${prefix}${name}`;
			return {
				path,
				bytes: await this.blob(repository, url, files.get(path), path, maxBytes, signal)
			};
		});
	}
	/** Wait until every running Git command has ended; the lifetime signal stops them. */
	async settled() {
		await Promise.allSettled([...this.running]);
	}
	/**
	* Serialize operations on one repository, so a fetch and the reads after it never interleave, and delete the
	* repository when an operation fails or is cancelled for any reason but a file over its read's size.
	*/
	locked(url, operation) {
		const repository = join(this.options.dir, createHash("sha256").update(url).digest("hex").slice(0, 24));
		return this.serialized(repository, () => operation(repository).catch(async (error) => {
			if (!(error instanceof FileTooLarge)) await rm(repository, {
				recursive: true,
				force: true
			});
			throw error;
		}));
	}
	/**
	* Run an operation after every earlier operation on the same repository directory has ended, in this service and in
	* other processes that share the cache directory, such as Web and Desktop on one profile.
	*/
	serialized(repository, operation) {
		const next = (this.locks.get(repository) ?? Promise.resolve()).then(async () => {
			await mkdir(this.options.dir, { recursive: true });
			return await withFileLock(repository, operation, {
				waitMs: OPERATION_COMMANDS * this.options.timeoutMs,
				signal: this.options.lifetime
			});
		});
		const tracked = next.then(() => void 0, () => void 0).finally(() => {
			if (this.locks.get(repository) === tracked) this.locks.delete(repository);
			this.running.delete(tracked);
		});
		this.locks.set(repository, tracked);
		this.running.add(tracked);
		return next;
	}
	/** The commit to read: a fetched commit, fetching it or the default branch when necessary. */
	async commit(repository, url, commit, signal) {
		if (!this.gitStarts) {
			await this.run(this.options.dir, ["--version"], signal, 1024).catch((error) => {
				throw new DetailFailure("git-unavailable", error.message, { cause: error });
			});
			this.gitStarts = true;
		}
		if (!existsSync(repository)) {
			const staging = await mkdtemp(join(this.options.dir, STAGING_PREFIX));
			try {
				await this.run(staging, [
					"init",
					"--quiet",
					"--bare",
					"--template="
				], signal);
				for (const [key, value] of [
					["remote.origin.url", url],
					["remote.origin.promisor", "true"],
					["remote.origin.partialclonefilter", "blob:none"]
				]) await this.run(staging, [
					"config",
					key,
					value
				], signal);
				await rename(staging, repository);
			} finally {
				await rm(staging, {
					recursive: true,
					force: true
				});
			}
		}
		await utimes(repository, /* @__PURE__ */ new Date(), /* @__PURE__ */ new Date());
		const head = this.heads.get(url);
		const known = commit ?? (head !== void 0 && head.expiresAt > Date.now() ? head.value : void 0);
		if (known !== void 0 && !(await this.git([
			"show-ref",
			"--verify",
			"--quiet",
			`refs/dsh/${known}`
		], repository, signal)).failed) return known;
		await this.fetch(repository, url, commit ?? "HEAD", signal);
		const fetched = commit ?? (await this.run(repository, ["rev-parse", "FETCH_HEAD"], signal)).toString("utf8").trim();
		await this.run(repository, [
			"update-ref",
			`refs/dsh/${fetched}`,
			fetched
		], signal);
		if (commit === void 0) this.heads.set(url, {
			value: fetched,
			expiresAt: Date.now() + this.options.ttlMs
		});
		return fetched;
	}
	/** Fetch one commit without file contents. */
	async fetch(repository, url, target, signal) {
		const args = [
			"fetch",
			"--quiet",
			"--depth",
			"1",
			"--filter=blob:none",
			"--no-tags",
			"origin",
			target
		];
		await this.watched(repository, url, args, signal, `${url}: git fetch`);
		await this.evict(repository);
	}
	/**
	* Run a Git command that may download objects, stopping it when the repository grows past its limit during or by the
	* command; the caller's lock then deletes the repository.
	*/
	async watched(repository, url, args, signal, label, maxBuffer = this.options.maxRepositoryBytes) {
		const limit = this.options.maxRepositoryBytes;
		const before = await sizeOf(repository);
		const oversized = new AbortController();
		const timer = setInterval(() => {
			sizeOf(repository).then(
				(size) => {
					if (size - before > limit) oversized.abort();
				},
				/* v8 ignore next -- Git renamed or
				deleted a file during the walk, which no test can time; the next tick measures again */
				(_racing) => {}
			);
		}, this.options.sizePollMs);
		let result;
		try {
			result = await this.git(args, repository, signal, oversized.signal, maxBuffer);
		} finally {
			clearInterval(timer);
		}
		if (oversized.signal.aborted || !result.failed && await sizeOf(repository) - before > limit) throw new DetailFailure("too-large", `${url}: one fetch adds more than ${String(limit)} bytes`);
		if (result.failed && !result.overflowed) throw remoteFailureOf(result, label);
		return result;
	}
	/**
	* Delete least recently used repositories until the cache fits, `kept` last, and staging directories older than the
	* Git deadline. Each deletion is queued behind the operations on its repository and not awaited, because the caller
	* holds the lock of `kept` and another repository's queued operation may be waiting to evict `kept`.
	*/
	async evict(kept) {
		const now = Date.now();
		const entries = [];
		for (const entry of await readdir(this.options.dir, { withFileTypes: true })) {
			const path = join(this.options.dir, entry.name);
			try {
				const used = (await stat(path)).mtimeMs;
				if (entry.name.startsWith(STAGING_PREFIX)) {
					if (now - used > this.options.timeoutMs) await rm(path, {
						recursive: true,
						force: true
					});
				} else if (entry.isDirectory()) entries.push({
					path,
					size: await sizeOf(path),
					used
				});
			} catch (_removed) {}
		}
		let total = entries.reduce((sum, entry) => sum + entry.size, 0);
		for (const entry of entries.sort((a, b) => Number(a.path === kept) - Number(b.path === kept) || a.used - b.used)) {
			if (total <= this.options.maxCacheBytes) return;
			this.serialized(entry.path, () => rm(entry.path, {
				recursive: true,
				force: true
			})).catch(
				/* v8 ignore next -- a
				forced recursive delete fails only on file permissions no test sets; the repository stays and is measured again
				after the next fetch */
				(_unremovable) => {}
			);
			total -= entry.size;
		}
	}
	/**
	* The regular files directly below a tree path, or the file at a path, with their blob ids; listing reads only trees,
	* so it fetches no file contents.
	*/
	async files(repository, commit, path, signal) {
		const listing = (await this.run(repository, [
			"ls-tree",
			"-z",
			commit,
			...path === void 0 ? [] : ["--", path]
		], signal)).toString("utf8");
		const files = /* @__PURE__ */ new Map();
		for (const line of listing.split("\0")) {
			const fields = /^100(?:644|755) blob ([0-9a-f]+)\t(.+)$/su.exec(line);
			if (fields !== null) files.set(fields[2], fields[1]);
		}
		return files;
	}
	/** Read a blob, fetching its contents under the repository size limit when the clone does not hold them yet. */
	async blob(repository, url, blob, path, maxBytes, signal) {
		const result = await this.watched(repository, url, [
			"cat-file",
			"blob",
			blob
		], signal, `git cat-file ${path}`, maxBytes);
		if (result.overflowed) throw new FileTooLarge(path, maxBytes);
		await this.evict(repository);
		return result.stdout;
	}
	/**
	* Run a Git command on local objects and return its output. A failure, such as a missing Git, a full disk, or a timeout,
	* is a plain error: it says nothing about the repository, so no read caches it.
	*/
	async run(repository, args, signal, maxBuffer) {
		const result = await this.git(args, repository, signal, void 0, maxBuffer);
		if (result.failed) throw new Error(`git ${args[0]}: ${result.diagnostic}`);
		return result.stdout;
	}
	/**
	* Run Git with options that keep it from running repository-supplied programs and allow only the configured transports.
	* Cancellation by `signal` or the lifetime rejects with its reason; `stop` ends Git with a failed result.
	*/
	async git(args, cwd, signal, stop, maxBuffer = this.options.maxRepositoryBytes) {
		const result = await execa("git", [...[
			"core.askPass=",
			"core.hooksPath=/dev/null",
			"core.fsmonitor=false",
			"submodule.recurse=false",
			"gc.auto=0",
			"maintenance.auto=false",
			"protocol.allow=never",
			...this.options.transports.map((transport) => `protocol.${transport}.allow=always`)
		].flatMap((setting) => ["-c", setting]), ...args], {
			cwd,
			env: gitEnv(),
			extendEnv: false,
			stdin: "ignore",
			encoding: "buffer",
			stripFinalNewline: false,
			reject: false,
			maxBuffer: {
				stdout: maxBuffer,
				stderr: this.options.maxRepositoryBytes
			},
			timeout: this.options.timeoutMs,
			cancelSignal: AbortSignal.any([
				signal,
				this.options.lifetime,
				...stop === void 0 ? [] : [stop]
			]),
			killDescendants: true,
			killSignal: "SIGKILL"
		});
		signal.throwIfAborted();
		this.options.lifetime.throwIfAborted();
		return {
			stdout: Buffer.from(result.stdout),
			failed: result.failed,
			overflowed: result.isMaxBuffer && result.stdout.byteLength >= maxBuffer,
			diagnostic: `${String(result.shortMessage)}\n${Buffer.from(result.stderr).toString("utf8").trim()}`
		};
	}
};
//#endregion
//#region lib/types/parse-thread.js
/** Runs index parsing in a fresh Worker Thread, so a large index does not block the Host event loop. */
function workerSpawn() {
	/* v8 ignore next -- built-artifact runs own the bundled path. */
	if (!import.meta.url.endsWith(".ts")) return {
		entry: new URL("./worker.cjs", import.meta.url),
		options: { execArgv: [] }
	};
	const bootstrap = [
		`import { register as registerEsm } from ${JSON.stringify(import.meta.resolve("tsx/esm/api"))}`,
		`import { register as registerCjs } from ${JSON.stringify(import.meta.resolve("tsx/cjs/api"))}`,
		"registerCjs()",
		"registerEsm()",
		`await import(${JSON.stringify(new URL("./worker.ts", import.meta.url).href)})`
	].join("\n");
	return {
		entry: new URL(`data:text/javascript,${encodeURIComponent(bootstrap)}`),
		options: { execArgv: [] }
	};
}
/** Whether a Worker message is a parse result: an index object, or a violation's path, reason, and message. */
function isResult(value) {
	if (typeof value !== "object" || value === null) return false;
	const result = value;
	return result.ok === true ? typeof result.index === "object" && result.index !== null : result.ok === false && [
		result.path,
		result.reason,
		result.message
	].every((field) => typeof field === "string");
}
/**
* Run `parseIndexBody` in a fresh Worker Thread. The body's buffer is transferred to the Worker and is unusable
* afterwards; the result is copied back by structured clone.
* @param body - the complete response body; it must own its whole buffer.
* @param url - the index URL.
* @param signal - terminates the Worker and rejects with the signal's reason.
* @returns the Worker's result.
* @throws the abort reason, or an Error when the Worker fails, exits without a result, or reports an unexpected value.
*/
async function parseIndexInWorker(body, url, signal) {
	signal.throwIfAborted();
	const { entry, options } = workerSpawn();
	const request = {
		body,
		url
	};
	const worker = new Worker(entry, {
		...options,
		workerData: request,
		transferList: [body.buffer]
	});
	return await new Promise((resolve, reject) => {
		let settled = false;
		const settle = (finish) => {
			if (settled) return;
			settled = true;
			signal.removeEventListener("abort", abort);
			worker.terminate().then(finish, finish);
		};
		const abort = () => {
			settle(() => {
				reject(signal.reason);
			});
		};
		signal.addEventListener("abort", abort, { once: true });
		worker.once("message", (value) => {
			settle(() => {
				if (isResult(value)) resolve(value);
				else reject(/* @__PURE__ */ new Error("index parser reported an unexpected value"));
			});
		});
		worker.once("error", (error) => {
			settle(() => {
				reject(error);
			});
		});
		worker.once("exit", (code) => {
			settle(() => {
				reject(/* @__PURE__ */ new Error(`index parser exited before reporting a result (code ${String(code)})`));
			});
		});
	});
}
//#endregion
//#region lib/types/search.js
/** Filtering, fuzzy text matching, and ordering of one validated index's plugins. */
/**
* Letters, marks, and digits used by CJK scripts, which write words without spaces between them; script extensions
* include shared characters such as the prolonged sound mark ー and the iteration mark 々.
*/
const CJK = String.raw`[[\p{scx=Han}\p{scx=Hira}\p{scx=Kana}\p{scx=Hang}]&&[\p{L}\p{M}\p{N}]]`;
/** A run of CJK characters, or a run of other letters, marks, and digits. */
const RUN = new RegExp(String.raw`${CJK}+|[[\p{L}\p{M}\p{N}]--${CJK}]+`, "gv");
const CJK_START = new RegExp(String.raw`^${CJK}`, "v");
/**
* Split NFKC-normalized text into search terms, so composed and decomposed forms, half-width kana, and compatibility
* characters yield the same terms. A CJK run yields its overlapping character pairs and its last character, so a query
* matches without word segmentation and every character of the run starts some term.
* @param text - indexed or queried text.
* @returns the terms in text order, not yet lowercased.
*/
function searchTerms(text) {
	return [...text.normalize("NFKC").matchAll(RUN)].flatMap(([run]) => {
		if (!CJK_START.test(run)) return [run];
		const chars = Array.from(run);
		return [...chars.slice(1).map((char, position) => `${chars[position]}${char}`), chars.at(-1)];
	});
}
/** Every language variant of a localized text. */
function variants(text) {
	if (text === void 0) return [];
	return typeof text === "string" ? [text] : Object.values(text);
}
/** Plugins indexed per event-loop turn while the text index is built. */
const BUILD_CHUNK = 500;
/** Matches queries against one index; the text index is built on the first query with text, yielding between chunks. */
var MarketSearch = class {
	index;
	engine;
	/** @param index - the validated index to search. */
	constructor(index) {
		this.index = index;
	}
	/**
	* Select and order the plugins matching a query.
	* @param selection - text, filters, and order.
	* @returns every matching plugin in result order.
	*/
	async select(selection) {
		const plugins = this.index.plugins;
		const text = selection.text ?? "";
		const scores = /* @__PURE__ */ new Map();
		let positions;
		if (searchTerms(text).length === 0) positions = plugins.map((_plugin, position) => position);
		else {
			for (const result of await this.search(text)) scores.set(result.id, result.score);
			positions = [...scores.keys()].sort((a, b) => a - b);
		}
		const labels = selection.labels ?? [];
		const matching = positions.filter((position) => {
			const { metadata } = plugins[position];
			return (selection.category === void 0 || metadata.category === selection.category) && labels.every((label) => metadata.labels.includes(label));
		});
		return this.order(matching, selection.sort, scores).map((position) => plugins[position]);
	}
	async search(text) {
		this.engine ??= this.build();
		return (await this.engine).search(text);
	}
	async build() {
		const { categories, labels } = this.index.metadata;
		const engine = new MiniSearch({
			fields: [
				"name",
				"title",
				"keywords",
				"description",
				"author",
				"taxonomy"
			],
			storeFields: [],
			tokenize: searchTerms,
			processTerm: (term) => term.toLowerCase(),
			searchOptions: {
				boost: {
					name: 3,
					title: 3,
					keywords: 2
				},
				prefix: true,
				fuzzy: (term) => term.length >= 4 ? .2 : false,
				maxFuzzy: 2,
				combineWith: "AND"
			}
		});
		await engine.addAllAsync(this.index.plugins.map((plugin, id) => {
			const { category, keywords } = plugin.metadata;
			const title = (declarations, id) => variants(declarations[id].title);
			const taxonomy = [...category === void 0 ? [] : title(categories, category), ...plugin.metadata.labels.flatMap((label) => title(labels, label))];
			return {
				id,
				name: plugin.name,
				title: variants(plugin.title).join("\n"),
				keywords: keywords.join("\n"),
				description: variants(plugin.description).join("\n"),
				author: plugin.author ?? "",
				taxonomy: taxonomy.join("\n")
			};
		}), { chunkSize: BUILD_CHUNK });
		return engine;
	}
	order(positions, sort, scores) {
		switch (sort.kind) {
			case "index": return positions;
			case "relevance": return positions.sort((a, b) => (scores.get(b) ?? 0) - (scores.get(a) ?? 0) || a - b);
			case "published":
			case "key": {
				const value = (position) => {
					const plugin = this.index.plugins[position];
					if (sort.kind === "published") return Math.max(...plugin.versions.map((version) => version.publishTimestamp));
					return Object.hasOwn(plugin.metadata.sortKeys, sort.id) ? plugin.metadata.sortKeys[sort.id] : void 0;
				};
				const sign = sort.direction === "asc" ? 1 : -1;
				return positions.map((position) => ({
					position,
					value: value(position)
				})).sort((a, b) => {
					if (a.value === void 0 || b.value === void 0) return a.value === b.value ? a.position - b.position : a.value === void 0 ? 1 : -1;
					return (a.value - b.value) * sign || a.position - b.position;
				}).map((item) => item.position);
			}
			/* v8 ignore next -- the Gateway validates `sort` against the closed union */
			default: return assertNever(sort);
		}
	}
};
//#endregion
//#region lib/types/index.js
/** Host reader of configured plugin market indexes for the Discover page and the `plugin_market` tool. */
var __runInitializers = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
	function accept(f) {
		if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected");
		return f;
	}
	var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
	var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
	var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
	var _, done = false;
	for (var i = decorators.length - 1; i >= 0; i--) {
		var context = {};
		for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
		for (var p in contextIn.access) context.access[p] = contextIn.access[p];
		context.addInitializer = function(f) {
			if (done) throw new TypeError("Cannot add initializers after decoration has completed");
			extraInitializers.push(accept(f || null));
		};
		var result = (0, decorators[i])(kind === "accessor" ? {
			get: descriptor.get,
			set: descriptor.set
		} : descriptor[key], context);
		if (kind === "accessor") {
			if (result === void 0) continue;
			if (result === null || typeof result !== "object") throw new TypeError("Object expected");
			if (_ = accept(result.get)) descriptor.get = _;
			if (_ = accept(result.set)) descriptor.set = _;
			if (_ = accept(result.init)) initializers.unshift(_);
		} else if (_ = accept(result)) if (kind === "field") initializers.unshift(_);
		else descriptor[key] = _;
	}
	if (target) Object.defineProperty(target, contextIn.name, descriptor);
	done = true;
};
/** The exact diagnostic of a failure, including a non-Error rejection. */
function messageOf(error) {
	return error instanceof Error ? error.message : String(error);
}
/** Causes followed below a rejection; a deeper or cyclic chain is cut there. */
const MAX_CAUSES = 4;
/** A rejection with its cause chain: a Node `fetch` failure is a bare `fetch failed` whose reason is its cause. */
function causeChainOf(error, depth = 0) {
	return error instanceof Error && error.cause !== void 0 && depth < MAX_CAUSES ? `${error.message}: ${causeChainOf(error.cause, depth + 1)}` : messageOf(error);
}
/** The Fetch route serving images beside detail documents. */
const DETAIL_FILE_PATH = "/api/plugin-market/detail-file";
/** Registries whose packages the public npm file CDNs serve. */
const PUBLIC_REGISTRIES = new Set(["registry.npmjs.org", "registry.npmmirror.com"]);
/** Largest page `search` returns. */
const MAX_LIMIT = 100;
let PluginMarket = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _listIndexes_decorators;
	let _setIndexes_decorators;
	let _search_decorators;
	let _readEntryDetail_decorators;
	return class PluginMarket extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_listIndexes_decorators = [Remote];
			_setIndexes_decorators = [Remote];
			_search_decorators = [Remote];
			_readEntryDetail_decorators = [Remote];
			__esDecorate(this, null, _listIndexes_decorators, {
				kind: "method",
				name: "listIndexes",
				static: false,
				private: false,
				access: {
					has: (obj) => "listIndexes" in obj,
					get: (obj) => obj.listIndexes
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _setIndexes_decorators, {
				kind: "method",
				name: "setIndexes",
				static: false,
				private: false,
				access: {
					has: (obj) => "setIndexes" in obj,
					get: (obj) => obj.setIndexes
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _search_decorators, {
				kind: "method",
				name: "search",
				static: false,
				private: false,
				access: {
					has: (obj) => "search" in obj,
					get: (obj) => obj.search
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _readEntryDetail_decorators, {
				kind: "method",
				name: "readEntryDetail",
				static: false,
				private: false,
				access: {
					has: (obj) => "readEntryDetail" in obj,
					get: (obj) => obj.readEntryDetail
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			if (_metadata) Object.defineProperty(this, Symbol.metadata, {
				enumerable: true,
				configurable: true,
				writable: true,
				value: _metadata
			});
		}
		config = __runInitializers(this, _instanceExtraInitializers);
		static inject = ["pluginManager"];
		static Config = z.object({
			indexes: z.array(z.string().pattern(/^https?:\/\/\S+$/u)).default([]).volatile(),
			fetchTimeoutMs: z.natural().min(1).max(MAX_TIMER_DELAY_MS).default(15e3),
			maxBytes: z.natural().min(1).default(128 * 1024 * 1024),
			cacheTtlMs: z.natural().default(6e5),
			detailMaxBytes: z.natural().min(1).default(1024 * 1024),
			detailImageMaxBytes: z.natural().min(1).default(5 * 1024 * 1024),
			detailRepositoryMaxBytes: z.natural().min(1).default(50 * 1024 * 1024),
			detailCacheMaxBytes: z.natural().min(1).default(500 * 1024 * 1024),
			detailCacheDir: z.string(),
			detailGitTimeoutMs: z.natural().min(1).max(MAX_TIMER_DELAY_MS).default(6e4),
			detailNpmFileSources: z.array(z.union(NPM_FILE_SOURCES)).min(1).default([...NPM_FILE_SOURCES]),
			detailGitlabHosts: z.array(z.string()).default(["gitlab.com"])
		});
		lifetime = new AbortController();
		cache = /* @__PURE__ */ new Map();
		pending = /* @__PURE__ */ new Map();
		errors = /* @__PURE__ */ new Map();
		entryId;
		repositories;
		details;
		constructor(ctx, config) {
			super(ctx, "pluginMarket");
			this.config = config;
			this.entryId = ctx.fiber.entry?.options.id;
			const profile = ctx.get("profileContext");
			const cacheDir = profile === void 0 ? config.detailCacheDir : resolve(profile.dir, config.detailCacheDir ?? join(".plugin-market", "repositories"));
			if (cacheDir === void 0 || !isAbsolute(cacheDir)) throw new Error("plugin-market: detailCacheDir must be an absolute path when the service runs without a dsh profile");
			const lifetime = this.lifetime.signal;
			this.repositories = new RepositoryCache({
				dir: cacheDir,
				maxRepositoryBytes: config.detailRepositoryMaxBytes,
				maxCacheBytes: config.detailCacheMaxBytes,
				timeoutMs: config.detailGitTimeoutMs,
				ttlMs: config.cacheTtlMs,
				transports: [
					"https",
					"http",
					"ssh"
				],
				lifetime,
				sizePollMs: 200
			});
			this.details = new DetailReader({
				maxDocumentBytes: config.detailMaxBytes,
				maxImageBytes: config.detailImageMaxBytes,
				ttlMs: config.cacheTtlMs
			}, this.detailRepositoryOptions());
			ctx.inject(["connection"], (scope) => {
				scope.effect(() => scope.connection.fetch.register({
					path: DETAIL_FILE_PATH,
					methods: ["GET"],
					requestBody: "buffered",
					fetch: (request) => this.serveDetailFile(request)
				}), `plugin-market: ${DETAIL_FILE_PATH}`);
			});
			ctx.on("loader/volatile-update", () => {
				const configured = new Set(this.sources().flatMap((source) => source.error === void 0 ? [source.url] : []));
				for (const url of [...this.cache.keys(), ...this.errors.keys()]) if (!configured.has(url)) {
					this.cache.delete(url);
					this.errors.delete(url);
				}
				ctx.emit("plugin-market/changed");
			});
			ctx.effect(() => async () => {
				this.lifetime.abort(/* @__PURE__ */ new Error("Plugin market disposed"));
				await Promise.allSettled([...this.pending.values(), this.repositories.settled()]);
			});
		}
		/**
		* List the configured index sources without fetching them.
		* @returns every source in order, with the title of its last successful read and, when the last read failed, its error.
		*/
		listIndexes() {
			return this.sources().map(({ url, error: invalid }) => {
				const index = this.cache.get(url)?.document.index;
				const error = invalid ?? this.errors.get(url);
				return {
					url,
					...index === void 0 ? {} : {
						title: index.title,
						...index.description === void 0 ? {} : { description: index.description }
					},
					...error === void 0 ? {} : { error }
				};
			});
		}
		/**
		* Replace the configured index sources through Settings, without remounting this service.
		* `plugin-market/changed` announces the committed list. A URL already configured is kept as written, so a client can
		* add or remove sources beside one that `listIndexes` reports as unusable.
		* @param urls - index URLs in display order.
		* @throws {RemoteError} `plugin-market/invalid-index-url` for an invalid new URL or a repeated URL;
		* `plugin-market/settings-unavailable` without Settings or a profile entry.
		*/
		async setIndexes(urls) {
			const configured = new Set(this.config.indexes.get());
			const normalized = urls.map((url) => configured.has(url) ? url : normalizeIndexUrl(url));
			const keys = normalized.map((url) => URL.canParse(url) ? new URL(url).href : url);
			const repeated = normalized.find((_url, position) => keys.indexOf(keys[position]) !== position);
			if (repeated !== void 0) throw new RemoteError("plugin-market/invalid-index-url", `Index source listed twice: ${repeated}`, {
				url: repeated,
				reason: "duplicate"
			});
			const settings = this.ctx.get("settings");
			if (settings === void 0 || this.entryId === void 0) throw new RemoteError("plugin-market/settings-unavailable", "Index sources require the settings service and a profile entry", {});
			await settings.update(this.entryId, { indexes: normalized });
		}
		/**
		* Return one page of the plugins of a configured index that match a query. Concurrent reads of a URL share one
		* fetch; a successful document is reused for `cacheTtlMs` unless `refresh` is set. Matching and ordering read only
		* the document; installation status and compatibility are computed for the returned page on every call.
		* @param url - a configured index URL.
		* @param query - text, filters, order, page, and whether to bypass the cached document.
		* @returns the index header, the page's plugins with each plugin's default version and installation status and each
		*   version's compatibility and install arguments, the number of matches, and the next page's offset.
		* @throws {RemoteError} `plugin-market/invalid-query`, `plugin-market/unknown-index`, `plugin-market/invalid-index-url`,
		*   `plugin-market/fetch-failed`, or `plugin-market/invalid-index`.
		*/
		async search(url, query) {
			if (!Number.isSafeInteger(query.offset) || query.offset < 0) throw new RemoteError("plugin-market/invalid-query", "offset must be a non-negative integer", {
				field: "offset",
				reason: "range"
			});
			if (!Number.isInteger(query.limit) || query.limit < 1 || query.limit > MAX_LIMIT) throw new RemoteError("plugin-market/invalid-query", `limit must be an integer from 1 to ${String(MAX_LIMIT)}`, {
				field: "limit",
				reason: "range"
			});
			const document = await this.document(url, query.refresh === true, query.fetchedAt);
			const matching = await document.search.select(query);
			const page = matching.slice(query.offset, query.offset + query.limit);
			const listing = await this.annotator();
			const next = query.offset + page.length;
			const { index } = document;
			return {
				url,
				title: index.title,
				...index.description === void 0 ? {} : { description: index.description },
				metadata: index.metadata,
				fetchedAt: document.fetchedAt,
				entries: page.map(listing),
				total: matching.length,
				nextOffset: next < matching.length ? next : null
			};
		}
		/**
		* Read one plugin of a configured index, using the cached document as `search` does.
		* @param url - a configured index URL.
		* @param name - the plugin's package name.
		* @returns the plugin with its default version, installation status, and annotated versions; undefined when the
		*   index does not list it.
		* @throws {RemoteError} `plugin-market/unknown-index`, `plugin-market/invalid-index-url`, `plugin-market/fetch-failed`,
		*   or `plugin-market/invalid-index`.
		*/
		async entry(url, name) {
			const plugin = (await this.document(url, false)).index.plugins.find((candidate) => candidate.name === name);
			return plugin === void 0 ? void 0 : (await this.annotator())(plugin);
		}
		/**
		* Read the long description of one listed version of a plugin market entry, from the version's source: the npm file
		* CDNs for a public npm version, or its pinned Git commit. The Markdown file the package's locale `meta.detail` names
		* in `language` comes first, then the package's README, then the README of the entry's `repository`, or for an npm
		* version without one, of its `package.json` `repository`, at the default branch. An npm version from another
		* registry has no readable package files, so only the entry's `repository` is read.
		* @param url - a configured index URL.
		* @param name - the plugin's package name.
		* @param version - one listed version.
		* @param language - the display language, such as `zh-CN`; documents fall back through its prefixes to English.
		* @param options - `refresh` reads again instead of reusing a result cached for `cacheTtlMs`; `npmFileSources` names
		*   npm file CDNs to ask first for an npm version, in order; configured CDNs it omits follow, and CDNs that are not
		*   configured are not asked.
		* @returns the document with its origin and image document id, or `none`.
		* @throws {RemoteError} the failures of `search`, `plugin-market/unknown-plugin` for an unlisted plugin or version,
		*   `plugin-market/invalid-query`, or `plugin-market/detail-unavailable`.
		*/
		async readEntryDetail(url, name, version, language, options) {
			const languages = this.languages(language);
			const plugin = (await this.document(url, false)).index.plugins.find((candidate) => candidate.name === name);
			const listed = plugin?.versions.find((candidate) => candidate.version === version);
			if (plugin === void 0 || listed === void 0) throw new RemoteError("plugin-market/unknown-plugin", `${url} lists no ${name}@${version}`, {
				name,
				version
			});
			const declared = plugin.repository;
			const found = declared === void 0 ? void 0 : repositoryOf(declared.url);
			let repository = found === void 0 ? void 0 : {
				...found,
				directory: declared?.directory ?? ""
			};
			const { source } = listed;
			let location;
			if (source.type === "git") {
				const ref = repositoryOf(source.url);
				/* v8 ignore next -- parseMarketIndex admits only https Git URLs with an owner and a project */
				if (ref === void 0) return { status: "none" };
				const dir = source.path ?? "";
				if (repository?.url === ref.url && repository.directory === dir) repository = void 0;
				location = {
					files: repositoryFiles(ref, source.commit, this.detailRepositoryOptions()),
					dir,
					repository
				};
			} else if (source.registry === void 0 || PUBLIC_REGISTRIES.has(new URL(source.registry).host)) location = this.npmLocation(name, version, repository ?? "manifest", options?.npmFileSources ?? []);
			else location = {
				dir: "",
				repository
			};
			return await this.readDetail(location, languages, options?.refresh === true);
		}
		languages(language) {
			try {
				return languageChain(language);
			} catch (error) {
				throw new RemoteError("plugin-market/invalid-query", messageOf(error), {
					field: "language",
					reason: "format"
				}, { cause: error });
			}
		}
		detailRepositoryOptions() {
			const { config } = this;
			return {
				timeoutMs: config.fetchTimeoutMs,
				lifetime: this.lifetime.signal,
				gitlabHosts: config.detailGitlabHosts.map((host) => host.toLowerCase()),
				cache: this.repositories
			};
		}
		/** The detail location of a published npm version on the configured CDNs, the preferred ones first. */
		npmLocation(name, version, repository, preferred) {
			return {
				dir: "",
				repository,
				files: npmFiles(name, version, npmFileSourceOrder(this.config.detailNpmFileSources, preferred), {
					timeoutMs: this.config.fetchTimeoutMs,
					lifetime: this.lifetime.signal
				})
			};
		}
		async readDetail(location, languages, refresh) {
			try {
				return await this.details.read(location, languages, refresh);
			} catch (error) {
				if (!(error instanceof DetailFailure)) throw error;
				throw new RemoteError("plugin-market/detail-unavailable", error.message, { reason: error.reason }, { cause: error });
			}
		}
		/** Answer `GET ${DETAIL_FILE_PATH}?document=<id>&path=<destination>` with one image beside a read detail document. */
		async serveDetailFile(request) {
			const params = new URL(request.url).searchParams;
			const document = params.get("document");
			const path = params.get("path");
			const cache = `private, max-age=${String(Math.floor(this.config.cacheTtlMs / 1e3))}`;
			if (document === null || path === null) return detailImageResponse("invalid", cache);
			try {
				return detailImageResponse(await this.details.image(document, path), cache);
			} catch (error) {
				if (!(error instanceof DetailFailure)) throw error;
				return detailImageResponse(error.reason, cache);
			}
		}
		async document(url, refresh, fetchedAt) {
			const source = this.sources().find((candidate) => candidate.url === url);
			if (source === void 0) throw new RemoteError("plugin-market/unknown-index", `Not a configured index source: ${url}`, { url });
			if (source.error !== void 0) throw new RemoteError("plugin-market/invalid-index-url", source.error, {
				url,
				reason: "configured"
			});
			const cached = this.cache.get(url);
			const usable = cached !== void 0 && (cached.expiresAt > Date.now() || cached.document.fetchedAt === fetchedAt);
			return !refresh && usable ? cached.document : await this.fetchShared(url);
		}
		/** Annotate plugins with the profile's bundles and exemptions as they are now. */
		async annotator() {
			const [bundles, { exemptions }] = [await this.ctx.pluginManager.listBundles(), this.ctx.pluginManager.listVersionExemptions()];
			const runtimeVersion = getDshRuntimeVersion();
			return (plugin) => listingOf(plugin, bundles, exemptions, runtimeVersion);
		}
		/**
		* The configured sources as written, each with the refusal that makes it unusable. Settings and profile files can
		* write any http(s) string, so every use validates instead of the schema; an unusable source is listed with its error
		* and never fetched, and a repeated URL counts once.
		*/
		sources() {
			const seen = /* @__PURE__ */ new Set();
			return this.config.indexes.get().flatMap((url) => {
				if (seen.has(url)) return [];
				seen.add(url);
				try {
					return normalizeIndexUrl(url) === url ? [{ url }] : [{
						url,
						error: `Index URL is not in canonical form; use ${normalizeIndexUrl(url)}`
					}];
				} catch (error) {
					return [{
						url,
						error: messageOf(error)
					}];
				}
			});
		}
		configured(url) {
			return this.sources().some((source) => source.url === url && source.error === void 0);
		}
		fetchShared(url) {
			let pending = this.pending.get(url);
			if (pending === void 0) {
				pending = this.fetchIndex(url).then((index) => {
					const document = {
						index,
						fetchedAt: Math.max(Date.now(), (this.cache.get(url)?.document.fetchedAt ?? 0) + 1),
						search: new MarketSearch(index)
					};
					if (this.configured(url)) {
						this.cache.set(url, {
							document,
							expiresAt: Date.now() + this.config.cacheTtlMs
						});
						this.errors.delete(url);
					}
					return document;
				}, (error) => {
					if (this.configured(url)) this.errors.set(url, messageOf(error));
					throw error;
				}).finally(() => {
					this.pending.delete(url);
				});
				this.pending.set(url, pending);
			}
			return pending;
		}
		async fetchIndex(url) {
			const signal = AbortSignal.any([this.lifetime.signal, AbortSignal.timeout(this.config.fetchTimeoutMs)]);
			const failed = (reason, cause) => new RemoteError("plugin-market/fetch-failed", `Could not fetch ${url}: ${reason}`, {
				url,
				reason
			}, { cause });
			let response;
			try {
				response = await fetch(url, {
					signal,
					redirect: "error",
					headers: { accept: "application/json" }
				});
			} catch (error) {
				throw failed(causeChainOf(error), error);
			}
			if (!response.ok) {
				await response.body?.cancel();
				throw failed(`HTTP ${String(response.status)}`);
			}
			const result = await parseIndexInWorker(await this.readBody(response, failed), url, this.lifetime.signal).catch((error) => {
				throw failed(messageOf(error), error);
			});
			if (result.ok) return result.index;
			throw new RemoteError("plugin-market/invalid-index", `${url}: ${result.message}`, {
				url,
				path: result.path,
				reason: result.reason
			});
		}
		/** Read the whole body, refusing it as soon as it exceeds `maxBytes`. */
		async readBody(response, failed) {
			const limit = this.config.maxBytes;
			const tooLarge = `body exceeds ${String(limit)} bytes`;
			if (Number(response.headers.get("content-length") ?? 0) > limit) {
				await response.body?.cancel();
				throw failed(tooLarge);
			}
			const chunks = [];
			let size = 0;
			try {
				for await (const chunk of response.body ?? []) {
					size += chunk.byteLength;
					if (size > limit) throw failed(tooLarge);
					chunks.push(chunk);
				}
			} catch (error) {
				if (error instanceof RemoteError) throw error;
				throw failed(messageOf(error), error);
			}
			const body = new Uint8Array(size);
			let offset = 0;
			for (const chunk of chunks) {
				body.set(chunk, offset);
				offset += chunk.byteLength;
			}
			return body;
		}
	};
})();
function listingOf(plugin, bundles, exemptions, runtimeVersion) {
	const versions = plugin.versions.map((version) => ({
		...version,
		install: marketInstallTarget(plugin.name, version),
		compatibility: compatibilityOf(plugin.name, version, exemptions, runtimeVersion)
	}));
	const compatible = versions.filter((version) => version.compatibility.compatible);
	const stable = plugin.versions.some((version) => semver.prerelease(version.version) === null);
	const defaultVersion = compatible.find((version) => !stable || semver.prerelease(version.version) === null)?.version;
	/* v8 ignore next -- parseMarketIndex guarantees a non-empty version list */
	const measured = defaultVersion ?? versions[0]?.version ?? "";
	return {
		...plugin,
		versions,
		...defaultVersion === void 0 ? {} : { defaultVersion },
		installation: installationOf(plugin.name, measured, bundles)
	};
}
function installationOf(name, version, bundles) {
	const bundle = bundles.find((candidate) => candidate.name === name);
	if (bundle === void 0 || !bundle.installed && bundle.availability !== "installation") return { status: "not-installed" };
	if (!bundle.installed) return {
		status: "provided",
		...bundle.version === void 0 ? {} : { installedVersion: bundle.version }
	};
	if (bundle.version === void 0) return { status: "different-version" };
	const installedVersion = bundle.version;
	if (installedVersion === version) return {
		status: "installed",
		installedVersion
	};
	return {
		status: semver.valid(installedVersion) !== null && semver.gt(version, installedVersion) ? "update-available" : "different-version",
		installedVersion
	};
}
function compatibilityOf(name, version, exemptions, runtimeVersion) {
	const issue = evaluatePluginCompatibility({
		name,
		version: version.version,
		...version.engines.dsh === void 0 ? {} : { engines: { dsh: version.engines.dsh } }
	}, exemptions, runtimeVersion);
	if (issue === void 0) return { compatible: true };
	return {
		compatible: false,
		runtimeVersion: issue.runtimeVersion,
		requirements: issue.requirements,
		exempted: issue.exempted
	};
}
//#endregion
export { PluginMarket as default };
