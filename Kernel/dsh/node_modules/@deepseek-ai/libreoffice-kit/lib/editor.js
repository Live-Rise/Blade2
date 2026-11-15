import { createRequire } from "node:module";
import { copyFile, mkdir, mkdtemp, readFile, rm, stat, symlink, unlink, writeFile } from "node:fs/promises";
import { closeSync, constants, createWriteStream, fstatSync, lstatSync, openSync, readSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, extname, isAbsolute, join, posix, relative, resolve, sep, win32 } from "node:path";
import { pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";
import { create } from "fontkit";
import { Worker } from "node:worker_threads";
import { strFromU8, unzipSync } from "fflate";
import { SaxesParser } from "saxes";
import { spawn } from "node:child_process";
import { EventEmitter } from "node:events";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
//#region lib/types/font-file-identity.js
/** Preserve 64-bit device and inode values without JavaScript integer rounding. */
function fontFileIdentity(status) {
	return {
		dev: status.dev.toString(),
		ino: status.ino.toString(),
		size: Number(status.size),
		mtimeMs: Number(status.mtimeNs) / 1e6,
		ctimeMs: Number(status.ctimeNs) / 1e6
	};
}
/** Compare a source descriptor's exact identity with its indexed font metadata. */
function matchesFontFile(status, expected) {
	const current = fontFileIdentity(status);
	return [
		"dev",
		"ino",
		"size",
		"mtimeMs",
		"ctimeMs"
	].every((key) => current[key] === expected[key]);
}
//#endregion
//#region lib/types/fonts.js
/** Host font metadata snapshots and conversion-local glyph matching over original files. */
const VCL_WEIGHTS = [
	400,
	100,
	200,
	300,
	350,
	400,
	500,
	600,
	700,
	800,
	900
];
const GENERIC_FAMILIES = /* @__PURE__ */ new Set([
	"serif",
	"sansserif",
	"monospace",
	"cursive",
	"fantasy",
	"systemui",
	"symbol"
]);
/**
* @param error - Thrown filesystem value.
* @returns whether it means the path is not a usable font source.
*/
function absent(error) {
	return error instanceof Error && "code" in error && [
		"ENOENT",
		"EACCES",
		"EPERM",
		"ENOTDIR"
	].includes(String(error.code));
}
/**
* Case- and separator-insensitive family key used to compare names across sources.
* @param value - Family, style, or postscript name as recorded by a document or font.
* @returns The normalized key.
*/
function normalize(value) {
	return value.normalize("NFKC").toLowerCase().replaceAll(/[\s_-]/g, "");
}
/**
* Order named substitutions and their generic text family without excluding unlisted fonts.
* @param requested - Original family names, in document order.
* @param groups - Ordered family groups shared with Fontconfig aliases.
* @param pitch - VCL pitch; one requests a monospaced fallback.
* @param symbols - Whether symbol alternatives precede the generic fallback.
* @returns distinct family names, with original requests first.
*/
function fontFamilyPriority(requested, groups, pitch = 0, symbols = false) {
	const original = requested.map(normalize);
	const matched = groups.filter((group) => group.some((family) => original.includes(normalize(family))));
	const generic = original.includes("monospace") || pitch === 1 ? "monospace" : original.includes("serif") ? "serif" : matched.flat().map(normalize).find((family) => family === "serif" || family === "sansserif" || family === "monospace") ?? "sansserif";
	const families = [...requested, ...matched.flat()];
	for (const kind of symbols ? ["symbol", generic] : [generic]) for (const group of groups) if (group.some((family) => normalize(family) === kind)) families.push(...group);
	const priority = /* @__PURE__ */ new Map();
	for (const family of families) {
		const name = normalize(family);
		if (!priority.has(name)) priority.set(name, family);
	}
	return [...priority.values()];
}
/**
* Discover system and per-user font roots using the selected platform's path separators.
* @param platform - Host operating system.
* @param home - Host user home used to locate per-user font directories.
* @param env - Host environment containing Windows and XDG directory overrides.
* @returns candidate directories; missing platform directories are ignored during indexing.
*/
function systemFontDirectories(platform = process.platform, home = homedir(), env = process.env) {
	if (platform === "win32") return [win32.join(env.SystemRoot ?? "C:\\Windows", "Fonts"), win32.join(env.LOCALAPPDATA ?? win32.join(home, "AppData", "Local"), "Microsoft", "Windows", "Fonts")];
	if (platform === "darwin") return [
		"/System/Library/Fonts",
		"/Library/Fonts",
		posix.join(home, "Library/Fonts"),
		...mobileAssetFontDirectories("/System/Library/AssetsV2")
	];
	const shared = (env.XDG_DATA_DIRS ?? "/usr/local/share:/usr/share").split(":").filter(Boolean);
	return [.../* @__PURE__ */ new Set([
		...shared.map((path) => posix.join(path, "fonts")),
		posix.join(home, ".fonts"),
		posix.join(env.XDG_DATA_HOME ?? posix.join(home, ".local/share"), "fonts")
	])];
}
/**
* Font asset directories macOS publishes under its MobileAsset root.
* @param assets - Absolute MobileAsset font directory root.
* @returns matching asset directories, sorted by name.
* @throws when the root exists but cannot be read for a reason other than its absence or protection.
*/
function mobileAssetFontDirectories(assets) {
	let entries = [];
	try {
		entries = readdirSync(assets);
	} catch (error) {
		if (!absent(error)) throw error;
	}
	return entries.filter((name) => /^com_apple_MobileAsset_Font\d*$/.test(name)).sort().map((name) => join(assets, name));
}
/**
* @param parsed - Parsed font file: one face, or a collection of faces.
* @param face - Indexed metadata naming the face inside that file.
* @returns the physical face the index names.
* @throws when the file no longer provides the indexed face.
*/
function indexedFace(parsed, face) {
	const font = "fonts" in parsed ? parsed.fonts[face.faceIndex] : parsed;
	if (font === void 0 || font.postscriptName !== face.postscriptName) throw new Error("The indexed font face is no longer available; recreate the converter.");
	return font;
}
function regionalPriority(face, language) {
	const locale = language.toLowerCase();
	const preferred = locale.startsWith("ja") ? ["jp"] : locale.startsWith("ko") ? ["kr"] : locale.startsWith("zh") ? /hant|tw|hk|mo/.test(locale) ? ["tc", "hk"] : ["sc"] : [];
	if (preferred.length === 0) return 0;
	const region = `${face.family} ${face.postscriptName} ${basename(face.path)}`.toLowerCase().match(/(?:cjk|[\s_-])(sc|tc|jp|kr|hk)(?=[\s_.-]|$)/)?.[1];
	return region === void 0 ? 1 : preferred.includes(region) ? 0 : 2;
}
/** Conversion-local glyph coverage over one operation's metadata snapshot. */
var SystemFontCatalog = class {
	options;
	faces;
	queried = /* @__PURE__ */ new Map();
	byFile = /* @__PURE__ */ new Map();
	/**
	* @param options - Host metadata snapshot and fallback families.
	*/
	constructor(options) {
		this.options = options;
		this.faces = options.faces.map((face) => ({ ...face }));
		for (const face of this.faces) {
			const members = this.byFile.get(face.path) ?? [];
			members.push(face);
			this.byFile.set(face.path, members);
		}
	}
	/**
	* Find installed faces covering the requested family or missing characters.
	* Exact families precede configured alternatives, symbol families for requests without letters or numbers,
	* generic families, then other glyph-covering faces.
	* @param request - VCL family/style attributes and Unicode scalars missing from its current font.
	* @param signal - cancellation checked between synchronous font reads.
	* @returns selected physical files, deduplicated across collection faces; absent glyphs remain unresolved.
	* @throws if an indexed font changes or cannot be read during glyph matching.
	*/
	match(request, signal) {
		signal.throwIfAborted();
		const requested = request.family.split(";").map((family) => family.trim()).filter(Boolean);
		const primary = requested[0];
		let missingFamily;
		if (primary !== void 0 && !GENERIC_FAMILIES.has(normalize(primary)) && !this.faces.some((face) => face.aliases.includes(normalize(primary)))) missingFamily = primary;
		const symbols = request.codePoints.some((point) => /\p{Symbol}/u.test(String.fromCodePoint(point))) && !request.codePoints.some((point) => /[\p{Letter}\p{Number}]/u.test(String.fromCodePoint(point)));
		const priority = fontFamilyPriority(requested, this.options.fallbackFamilies, request.pitch, symbols).map(normalize);
		const missing = new Set(request.codePoints);
		const ranked = this.faces.map((face) => {
			const rank = priority.findIndex((family) => face.aliases.includes(family));
			const region = regionalPriority(face, request.language);
			const style = (request.style && normalize(request.style) === normalize(face.style) ? -1 : 0) + (request.italic === 3 || request.italic !== 0 === face.italic ? 0 : 100) + (request.pitch === 0 || request.pitch === 1 === face.fixed ? 0 : 50) + Math.abs((VCL_WEIGHTS[request.weight] ?? 400) - face.weight) / 100 + (request.width === 0 ? 0 : Math.abs(request.width - face.width));
			return {
				face,
				rank: rank < 0 ? priority.length : rank,
				region,
				style
			};
		}).sort((left, right) => left.rank - right.rank || left.region - right.region || left.style - right.style || left.face.path.localeCompare(right.face.path, "en") || left.face.faceIndex - right.face.faceIndex);
		const selected = [];
		const files = /* @__PURE__ */ new Set();
		for (const { face } of ranked) {
			signal.throwIfAborted();
			let covered;
			if (face.coverage !== void 0) {
				const coverage = face.coverage;
				covered = [...missing].filter((point) => coverage.some(([first, last]) => point >= first && point <= last));
			} else {
				let known = this.queried.get(face);
				if (known === void 0) {
					known = /* @__PURE__ */ new Map();
					this.queried.set(face, known);
				}
				const unknown = [...missing].filter((point) => !known.has(point));
				if (unknown.length > 0) {
					const parsed = create(readFont(face));
					const collection = "fonts" in parsed ? { fonts: parsed.fonts } : parsed;
					for (const member of this.byFile.get(face.path)) {
						let answers = this.queried.get(member);
						if (answers === void 0) {
							answers = /* @__PURE__ */ new Map();
							this.queried.set(member, answers);
						}
						const font = indexedFace(collection, member);
						try {
							for (const point of unknown) answers.set(point, font.hasGlyphForCodePoint(point));
						} catch {
							for (const point of unknown) answers.set(point, false);
						}
					}
				}
				covered = [...missing].filter((point) => known.get(point));
			}
			if (missing.size > 0 && covered.length === 0) continue;
			if (!files.has(face.path)) {
				selected.push(face);
				files.add(face.path);
			}
			for (const point of covered) missing.delete(point);
			if (missing.size === 0) break;
		}
		return {
			fonts: selected,
			...missingFamily === void 0 ? {} : { missingFamily },
			...missing.size === 0 ? {} : { unresolvedCodePoints: [...missing] }
		};
	}
};
/**
* Read the unchanged indexed regular file; reject concurrent replacement or truncation.
* @param face - Indexed face metadata whose identity and timestamps must still match the file.
* @returns the complete original font bytes.
* @throws when the file changed, was replaced, or was truncated since indexing.
*/
function readFont(face) {
	const fd = openSync(face.path, constants.O_RDONLY | constants.O_NONBLOCK | constants.O_NOFOLLOW);
	try {
		const before = fstatSync(fd, { bigint: true });
		if (!before.isFile() || !matchesFontFile(before, face)) throw new Error("An indexed font changed; recreate the converter.");
		const bytes = Buffer.alloc(face.size);
		for (let offset = 0; offset < bytes.length;) {
			const count = readSync(fd, bytes, offset, bytes.length - offset, offset);
			if (!count) throw new Error("An indexed font was truncated.");
			offset += count;
		}
		if (!matchesFontFile(fstatSync(fd, { bigint: true }), face)) throw new Error("An indexed font changed while reading.");
		return bytes;
	} finally {
		closeSync(fd);
	}
}
//#endregion
//#region lib/types/font-metadata-cache.js
/** Resolve the platform's user-local cache, independently of font discovery. */
function defaultFontMetadataCacheDirectory(platform = process.platform, home = homedir(), env = process.env) {
	if (platform === "darwin") return posix.join(home, "Library/Caches/libreoffice-kit");
	if (platform === "win32") return win32.join(env.LOCALAPPDATA ?? win32.join(home, "AppData/Local"), "libreoffice-kit/Cache");
	return posix.join(env.XDG_CACHE_HOME && posix.isAbsolute(env.XDG_CACHE_HOME) ? env.XDG_CACHE_HOME : posix.join(home, ".cache"), "libreoffice-kit");
}
//#endregion
//#region lib/types/options.js
const SANS_CJK_FAMILIES = [
	"Microsoft YaHei",
	"Microsoft YaHei UI",
	"微软雅黑",
	"PingFang SC",
	"Noto Sans CJK SC",
	"Noto Sans SC",
	"Source Han Sans SC",
	"SimHei",
	"黑体",
	"Heiti SC",
	"STHeiti"
];
/** Ordered family groups used when the caller supplies none. */
const FONT_FALLBACKS = [
	["Calibri", "Carlito"],
	[
		"Calibri Light",
		"Carlito",
		"Calibri"
	],
	["Cambria", "Caladea"],
	[
		"宋体",
		"SimSun",
		"NSimSun",
		"Songti SC",
		"STSong",
		"Noto Serif CJK SC",
		"Noto Serif SC",
		"Source Han Serif SC"
	],
	[
		"黑体",
		"SimHei",
		"Heiti SC",
		"STHeiti",
		"Noto Sans CJK SC",
		"Noto Sans SC",
		"Source Han Sans SC"
	],
	[
		"微软雅黑",
		"Microsoft YaHei",
		"Microsoft YaHei UI",
		"PingFang SC",
		"Noto Sans CJK SC",
		"Noto Sans SC"
	],
	[
		"楷体",
		"KaiTi",
		"Kaiti SC",
		"STKaiti",
		"LXGW WenKai"
	],
	[
		"仿宋",
		"FangSong",
		"STFangsong",
		"Songti SC",
		"Noto Serif CJK SC"
	],
	[
		"sans-serif",
		"Arial",
		"Liberation Sans",
		"Helvetica",
		"DejaVu Sans",
		"Calibri",
		"Calibri Light",
		"Carlito",
		...SANS_CJK_FAMILIES
	],
	[
		"serif",
		"Times New Roman",
		"Liberation Serif",
		"Times",
		"DejaVu Serif",
		"Cambria",
		"Caladea",
		"SimSun",
		"NSimSun",
		"宋体",
		"Songti SC",
		"STSong",
		"Noto Serif CJK SC",
		"Noto Serif SC",
		"Source Han Serif SC"
	],
	[
		"monospace",
		"Courier New",
		"Liberation Mono",
		"DejaVu Sans Mono",
		"Menlo",
		"Monaco",
		"Courier",
		"NSimSun",
		"Noto Sans Mono CJK SC",
		...SANS_CJK_FAMILIES
	],
	[
		"Symbol",
		"Standard Symbols PS",
		"Symbola",
		"Segoe UI Symbol",
		"Apple Symbols",
		"FreeSerif",
		"DejaVu Sans"
	]
];
const DEFAULT_LIMITS = {
	timeoutMs: 12e4,
	maxInputBytes: 67108864,
	maxOutputBytes: 134217728,
	maxImageResolution: 144,
	maxArchiveEntries: 2e4,
	maxUncompressedBytes: 536870912,
	maxFontFiles: 2e4,
	maxFontFileBytes: 268435456,
	maxLoadedFontBytes: 536870912,
	maxFontResolutionEntries: 4096,
	maxFontMetadataCacheBytes: 33554432
};
const FONT_COLLECTION_NAMES = ["fontDirectories", "initialFontFamilies"];
function positiveSafeInteger(value, name) {
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) throw new RangeError(`${name} must be a positive safe integer.`);
	if (name.endsWith("Ms") && value > 2147483647) throw new RangeError(`${name} exceeds the Node timer range.`);
	return value;
}
function fontNames(value, name) {
	if (!Array.isArray(value)) throw new TypeError(`${name} must contain nonempty strings.`);
	const names = [];
	for (const entry of value) {
		if (typeof entry !== "string" || entry.length === 0 || entry.includes("\0")) throw new TypeError(`${name} must contain nonempty strings.`);
		names.push(entry);
	}
	return names;
}
function fontGroups(value) {
	if (!Array.isArray(value) || value.some((group) => !Array.isArray(group) || group.length < 2 || group.some((entry) => typeof entry !== "string" || !entry.trim()))) throw new TypeError("fontFallbacks must contain groups of at least two nonblank font names.");
	return value.map((group) => [...group]);
}
/**
* Validate an optional caller-supplied engine package resolver.
* @param value - The `resolveEnginePackage` option value.
* @returns the resolver, or undefined when absent.
* @throws TypeError when the value is defined and not a function.
*/
function enginePackageResolver(value) {
	if (value !== void 0 && typeof value !== "function") throw new TypeError("resolveEnginePackage must be a function.");
	return value;
}
/**
* Resolve defaults once; reject misspelled options and malformed font settings.
* @param input - Caller options; absent fields keep their documented default.
* @returns The validated limits and font settings, with caller arrays copied.
* @throws TypeError for a non-object argument, an unknown option name, or a non-function `resolveEnginePackage`.
* @throws RangeError for a limit outside the positive safe-integer timer range.
*/
function resolveOptions(input = {}) {
	const callerOptions = input;
	if (callerOptions === null || typeof callerOptions !== "object" || Array.isArray(callerOptions)) throw new TypeError("Converter options must be an object.");
	const merged = {
		...DEFAULT_LIMITS,
		fontDirectories: systemFontDirectories(),
		fontFallbacks: FONT_FALLBACKS,
		initialFontFamilies: [],
		...input
	};
	for (const key of Object.keys(input)) if (!(key in DEFAULT_LIMITS) && !FONT_COLLECTION_NAMES.includes(key) && key !== "fontFallbacks" && key !== "fontMetadataCacheDirectory" && key !== "resolveEnginePackage") throw new TypeError(`Unknown converter option: ${key}`);
	enginePackageResolver(input.resolveEnginePackage);
	const cacheDirectory = input.fontMetadataCacheDirectory === void 0 ? defaultFontMetadataCacheDirectory() : input.fontMetadataCacheDirectory;
	if (cacheDirectory !== false && (typeof cacheDirectory !== "string" || !isAbsolute(cacheDirectory) || cacheDirectory.includes("\0"))) throw new TypeError("fontMetadataCacheDirectory must be an absolute path or false.");
	return {
		fontMetadataCacheDirectory: cacheDirectory,
		maxFontMetadataCacheBytes: positiveSafeInteger(merged.maxFontMetadataCacheBytes, "maxFontMetadataCacheBytes"),
		timeoutMs: positiveSafeInteger(merged.timeoutMs, "timeoutMs"),
		maxInputBytes: positiveSafeInteger(merged.maxInputBytes, "maxInputBytes"),
		maxOutputBytes: positiveSafeInteger(merged.maxOutputBytes, "maxOutputBytes"),
		maxImageResolution: positiveSafeInteger(merged.maxImageResolution, "maxImageResolution"),
		maxArchiveEntries: positiveSafeInteger(merged.maxArchiveEntries, "maxArchiveEntries"),
		maxUncompressedBytes: positiveSafeInteger(merged.maxUncompressedBytes, "maxUncompressedBytes"),
		maxFontFiles: positiveSafeInteger(merged.maxFontFiles, "maxFontFiles"),
		maxFontFileBytes: positiveSafeInteger(merged.maxFontFileBytes, "maxFontFileBytes"),
		maxLoadedFontBytes: positiveSafeInteger(merged.maxLoadedFontBytes, "maxLoadedFontBytes"),
		maxFontResolutionEntries: positiveSafeInteger(merged.maxFontResolutionEntries, "maxFontResolutionEntries"),
		fontDirectories: fontNames(merged.fontDirectories, "fontDirectories"),
		includeOfficeFonts: input.fontDirectories === void 0,
		initialFontFamilies: fontNames(merged.initialFontFamilies, "initialFontFamilies"),
		fontFallbacks: fontGroups(merged.fontFallbacks)
	};
}
//#endregion
//#region lib/types/engine.js
/** Resolve installed, version-matched engine packages without downloads or compilation. */
const require = createRequire(import.meta.url);
/** Exact compatible engine versions; unchanged platforms retain their published packages. */
const ENGINE_VERSIONS = Object.freeze({
	"darwin-arm64": "0.2.3",
	"darwin-x64": "0.2.3",
	"linux-arm64-glibc": "0.1.1",
	"linux-x64-glibc": "0.1.1",
	"win32-arm64": "0.2.3",
	"win32-x64": "0.2.3",
	wasm: "0.2.3"
});
/** npm scope and name prefix shared by the engine packages this adapter installs. */
const ENGINE_PREFIX = "@deepseek-ai/libreoffice-kit";
/**
* Whether an installed engine package directory is present on the resolution path.
* @param name - Engine package name.
* @returns true when at least one resolution directory holds the package.
*/
function installedPackageExists(name) {
	return (require.resolve.paths(name) ?? []).some((directory) => lstatSync(join(directory, name), { throwIfNoEntry: false }) !== void 0);
}
/**
* Return the optional package target for the current process ABI.
* @param platform - Host operating system.
* @param arch - Host CPU architecture.
* @param report - Host diagnostic report used to identify glibc.
* @returns the declared platform target, or undefined for an unsupported host.
*/
function platformTarget(platform = process.platform, arch = process.arch, report = hostReport) {
	if (!["arm64", "x64"].includes(arch)) return void 0;
	if (platform === "darwin" || platform === "win32") return `${platform}-${arch}`;
	if (platform !== "linux") return void 0;
	return report().header?.glibcVersionRuntime ? `linux-${arch}-glibc` : void 0;
}
/** @returns the host diagnostic report as the engine resolver reads it. */
function hostReport() {
	return process.report.getReport();
}
function asset(root, value) {
	if (typeof value !== "string" || !value || isAbsolute(value)) throw new Error("Engine manifest contains an invalid asset path.");
	const path = resolve(root, value);
	if (relative(root, path).startsWith(`..${sep}`) || relative(root, path) === "..") throw new Error("Engine asset escapes its installed package.");
	return path;
}
function glibcVersion(value) {
	return typeof value === "string" && /^(0|[1-9]\d*)\.(0|[1-9]\d*)(?:\.(0|[1-9]\d*))?$/.test(value) && value.split(".").every((part) => Number.isSafeInteger(Number(part))) ? value.split(".").map(Number) : void 0;
}
/** Resolve engine packages from this module's installation with Node module resolution. */
function installedEnginePackage(name) {
	return require.resolve(`${name}/package.json`);
}
/**
* Check one resolved engine `package.json` path before reading it.
* @param packageFile - Value the resolver returned.
* @param name - Engine package name.
* @returns the path unchanged.
* @throws Error when the value is not an absolute path to an existing file.
*/
function resolvedManifest(packageFile, name) {
	if (typeof packageFile !== "string") throw new Error(`Engine package resolver must synchronously return a string path for ${name}; it returned ${packageFile === null ? "null" : typeof packageFile}`);
	if (!isAbsolute(packageFile)) throw new Error(`Engine package resolver returned a non-absolute path for ${name}: ${String(packageFile)}`);
	let file;
	try {
		file = statSync(packageFile).isFile();
	} catch (error) {
		throw new Error(`Engine package resolver returned a missing path for ${name}: ${packageFile}`, { cause: error });
	}
	if (!file) throw new Error(`Engine package resolver returned a path that is not a file for ${name}: ${packageFile}`);
	return packageFile;
}
/**
* Prefer the installed compatible native engine; hosts without one use the shared WASM package.
* @param resolvePackage - Engine package manifest resolver; defaults to Node resolution from this module.
* @param packageExists - Whether a native package that resolved as `MODULE_NOT_FOUND` is nevertheless installed (and
* therefore incomplete). Only the default resolver probes this module's installation; a caller resolver is the sole
* authority on whether its packages exist.
* @param host - Process identification and diagnostic report.
* @returns the installed native engine, or the installed WASM engine.
*/
async function resolveEngine(resolvePackage = installedEnginePackage, packageExists = resolvePackage === installedEnginePackage ? installedPackageExists : () => false, { platform = process.platform, arch = process.arch, report = hostReport } = {}) {
	const details = platform === "linux" ? report() : {};
	const target = platformTarget(platform, arch, () => details);
	if (target !== void 0) {
		const name = `${ENGINE_PREFIX}-${target}`;
		let packageFile;
		try {
			packageFile = resolvedManifest(resolvePackage(name), name);
		} catch (error) {
			const failure = error;
			if (!(failure?.code === "MODULE_NOT_FOUND" && (resolvePackage !== installedEnginePackage || typeof failure.message === "string" && failure.message.includes(`${name}/package.json`)))) throw error;
			if (packageExists(name)) throw new Error(`Installed LibreOfficeKit package is incomplete: ${name}`, { cause: error });
		}
		if (packageFile !== void 0) {
			const engine = await readEngine(packageFile, "native", target);
			const minimum = glibcVersion(engine.glibcMinimum);
			const host = glibcVersion(details.header?.glibcVersionRuntime);
			if (!(minimum && host && minimum.some((part, index) => part > (host[index] ?? 0) && minimum.slice(0, index).every((prior, priorIndex) => prior === host[priorIndex])))) return engine;
		}
	}
	const wasm = `${ENGINE_PREFIX}-wasm`;
	return readEngine(resolvedManifest(resolvePackage(wasm), wasm), "wasm", "wasm");
}
async function engineAsset(root, value, name, kind) {
	const path = asset(root, value);
	const status = await stat(path);
	if (kind === "directory" ? !status.isDirectory() : !status.isFile()) throw new Error(`Installed LibreOfficeKit ${name} is not a ${kind}.`);
	if (name === "executable" && process.platform !== "win32" && !(status.mode & 73)) throw new Error("Installed LibreOfficeKit executable is not executable.");
	return path;
}
async function readEngine(packageFile, backend, target) {
	const root = dirname(packageFile);
	const [pkg, manifest] = await Promise.all([readFile(packageFile, "utf8").then((text) => JSON.parse(text)), readFile(resolve(root, "prebuilds.json"), "utf8").then((text) => JSON.parse(text))]);
	const engine = manifest.engine;
	if (pkg.name !== `${ENGINE_PREFIX}-${target}` || pkg.version !== ENGINE_VERSIONS[target] || manifest.version !== pkg.version || manifest.schemaVersion !== 1 || manifest.status !== "built" || engine === void 0 || engine.kind !== backend || manifest.platform !== target) throw new Error(`Installed LibreOfficeKit ${backend} package has an incompatible or incomplete manifest.`);
	const minimum = engine.glibcMinimum;
	if (minimum !== void 0 && (!target.endsWith("-glibc") || !glibcVersion(minimum))) throw new Error("Installed LibreOfficeKit engine has an invalid glibcMinimum.");
	const glibcFloor = minimum === void 0 ? {} : { glibcMinimum: minimum };
	if (backend === "native") {
		const executable = await engineAsset(root, engine.executable, "executable", "file");
		return {
			backend,
			root,
			programDirectory: await engineAsset(root, engine.programDirectory, "programDirectory", "directory"),
			executable,
			...["0.1.1", "0.1.5"].includes(String(pkg.version)) ? { legacyInputPath: true } : {},
			...glibcFloor
		};
	}
	return {
		backend,
		root,
		programDirectory: String(engine.programDirectory),
		loader: await engineAsset(root, engine.loader, "loader", "file"),
		wasm: await engineAsset(root, engine.wasm, "wasm", "file"),
		data: await engineAsset(root, engine.data, "data", "file"),
		metadata: await engineAsset(root, engine.metadata, "metadata", "file"),
		...glibcFloor
	};
}
//#endregion
//#region lib/types/errors.js
/** Stable render failure categories shared across the Node worker and native helper transports. */
const CODES = /* @__PURE__ */ new Set([
	"input-too-large",
	"output-too-large",
	"invalid-document",
	"unsupported-format",
	"invalid-output",
	"timeout",
	"unavailable",
	"failed"
]);
/** An actionable converter failure; operating-system errors and caller abort reasons remain unchanged. */
var ConversionError = class extends Error {
	/** Published failure category carried across worker and helper transports. */
	code;
	/**
	* @param code - Published failure category.
	* @param message - Consumer-facing failure description.
	* @param options - Standard error options; a cause is preserved for diagnostics.
	*/
	constructor(code, message, options) {
		super(message, options);
		this.name = "ConversionError";
		this.code = code;
	}
};
/**
* Only published categories cross worker/helper messages; unknown errors use the generic failure category.
* @param error - The thrown value, which may be any type and may carry an unpublished `code`.
* @returns The published category, or `failed` when no published category is present.
*/
function failureCode(error) {
	const code = error?.code;
	return typeof code === "string" && CODES.has(code) ? code : "failed";
}
//#endregion
//#region lib/types/font-snapshot-state.js
/** Shared in-flight metadata work with independent cancellation and joined Worker cleanup. */
/** Run one scan outside the event loop and join it before returning or cleaning its temporary file. */
async function runFontSnapshot(options, previous, signal) {
	signal.throwIfAborted();
	const temporaryPath = options.fontMetadataCacheDirectory === false ? void 0 : join(options.fontMetadataCacheDirectory, `.font-metadata-${randomUUID()}.tmp`);
	const data = {
		options,
		previous,
		...temporaryPath === void 0 ? {} : { temporaryPath }
	};
	const worker = new Worker(new URL("./font-snapshot-worker.js", import.meta.url), {
		workerData: data,
		execArgv: [],
		name: "libreoffice-font-metadata",
		stdout: true,
		stderr: true
	});
	worker.stdout.resume();
	worker.stderr.resume();
	const result = Promise.withResolvers();
	const abort = () => result.reject(signal.reason);
	signal.addEventListener("abort", abort, { once: true });
	worker.once("error", result.reject);
	worker.once("exit", (code) => result.reject(new ConversionError("failed", `Font metadata worker exited before returning a snapshot (${code}).`)));
	worker.once("message", (message) => {
		if (message.ok) result.resolve(message.snapshot);
		else result.reject(new ConversionError(failureCode(message), message.error));
	});
	try {
		return await result.promise;
	} finally {
		signal.removeEventListener("abort", abort);
		await worker.terminate();
		if (temporaryPath !== void 0) try {
			await rm(temporaryPath, { force: true });
		} catch {}
	}
}
/** Each acquisition refreshes inventory; overlapping acquisitions share only the active scan. */
var FontSnapshotState = class {
	previous = [];
	scan;
	/** Revalidate files for an operation without coupling its cancellation to other waiters. */
	async acquire(options, signal) {
		signal.throwIfAborted();
		while (this.scan?.controller.signal.aborted) {
			await this.scan.promise.catch(() => {});
			signal.throwIfAborted();
		}
		if (this.scan === void 0) {
			const controller = new AbortController();
			const promise = runFontSnapshot(options, this.previous, controller.signal).then((snapshot) => {
				this.previous = Buffer.byteLength(JSON.stringify(snapshot.records)) <= options.maxFontMetadataCacheBytes ? snapshot.records : [];
				return snapshot;
			}).finally(() => {
				this.scan = void 0;
			});
			this.scan = {
				controller,
				promise,
				waiters: 0
			};
		}
		const scan = this.scan;
		scan.waiters++;
		const cancelled = Promise.withResolvers();
		const abort = () => cancelled.reject(signal.reason);
		signal.addEventListener("abort", abort, { once: true });
		try {
			return await Promise.race([scan.promise, cancelled.promise]);
		} finally {
			signal.removeEventListener("abort", abort);
			scan.waiters--;
			if (scan.waiters === 0 && this.scan === scan) {
				scan.controller.abort(/* @__PURE__ */ new Error("All font scan waiters stopped."));
				await scan.promise.catch(() => {});
			}
		}
	}
	/** Release completed metadata after the owning factory has joined its operations. */
	clear() {
		this.previous = [];
	}
};
//#endregion
//#region lib/types/ooxml.js
/** Bounded OOXML inspection and declared font diagnostics, using maintained ZIP/XML parsers. */
const MAIN_PARTS = {
	odt: ["content.xml", "application/vnd.oasis.opendocument.text"],
	ods: ["content.xml", "application/vnd.oasis.opendocument.spreadsheet"],
	odp: ["content.xml", "application/vnd.oasis.opendocument.presentation"],
	docx: ["word/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"],
	xlsx: ["xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"],
	pptx: ["ppt/presentation.xml", "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"]
};
/**
* @param character - One code point as yielded by iteration over a string.
* @returns the Unicode scalar the character encodes; iteration never yields an empty string.
*/
function codePointOf(character) {
	return character.codePointAt(0);
}
/**
* Validate ZIP size and membership before decoding selected document XML.
* @param bytes - Complete source document bytes.
* @param extension - Input extension, which selects the main document part.
* @param limits - Archive entry and declared uncompressed byte limits.
* @returns declared font families and the scalar values readable document XML declares.
* @throws ConversionError when the extension is unsupported or the archive violates its limits.
*/
function inspectDocument$1(bytes, extension, limits) {
	const main = MAIN_PARTS[extension];
	if (!main) throw new ConversionError("unsupported-format", "Input extension must be docx, xlsx, or pptx.");
	const odf = [
		"odt",
		"ods",
		"odp"
	].includes(extension);
	const names = /* @__PURE__ */ new Set();
	let total = 0;
	let files;
	try {
		files = unzipSync(bytes, { filter(entry) {
			total += entry.originalSize;
			if (names.has(entry.name) || names.size >= limits.maxArchiveEntries || !Number.isSafeInteger(total) || total > limits.maxUncompressedBytes) throw new Error("OOXML archive exceeds its limits or repeats an entry.");
			names.add(entry.name);
			if (odf) return [
				"mimetype",
				"content.xml",
				"styles.xml",
				"META-INF/manifest.xml"
			].includes(entry.name);
			return entry.name === "[Content_Types].xml" || /^(word|xl|ppt)\/.*\.xml$/.test(entry.name) && !entry.name.endsWith("/fontTable.xml") && !entry.name.includes("/theme/");
		} });
	} catch (cause) {
		throw new ConversionError("invalid-document", "Input is not a supported, bounded OOXML archive.", { cause });
	}
	const contentTypes = files["[Content_Types].xml"];
	if (odf ? !names.has("content.xml") || !names.has("META-INF/manifest.xml") || strFromU8(files.mimetype ?? /* @__PURE__ */ new Uint8Array()) !== main[1] : !names.has("_rels/.rels") || !names.has(main[0]) || contentTypes === void 0 || !strFromU8(contentTypes).includes(main[1])) throw new ConversionError("invalid-document", `Input does not contain a ${extension} document.`);
	const families = /* @__PURE__ */ new Map();
	const codePoints = /* @__PURE__ */ new Set();
	for (const [name, data] of Object.entries(files)) {
		if (name === "[Content_Types].xml" || name === "mimetype" || name === "META-INF/manifest.xml") continue;
		const partFamilies = [];
		const partPoints = /* @__PURE__ */ new Set();
		const parser = new SaxesParser({ xmlns: true });
		parser.on("opentag", (tag) => {
			const word = /wordprocessingml\/(?:2006\/)?main$/.test(tag.uri) && tag.local === "rFonts";
			const drawing = /drawingml\/(?:2006\/)?main$/.test(tag.uri);
			const sheet = /spreadsheetml\/(?:2006\/)?main$/.test(tag.uri) && tag.local === "name";
			for (const attribute of Object.values(tag.attributes)) {
				if (odf && attribute.local === "font-family") partFamilies.push(attribute.value.replace(/^['"]|['"]$/g, ""));
				if (word && [
					"ascii",
					"hAnsi",
					"eastAsia",
					"cs"
				].includes(attribute.local) || drawing && attribute.local === "typeface" || sheet && attribute.local === "val") partFamilies.push(attribute.value);
			}
		});
		parser.on("text", (text) => {
			for (const char of text) if (!/\s/u.test(char)) partPoints.add(codePointOf(char));
		});
		const encoding = data[0] === 255 && data[1] === 254 ? "utf-16le" : data[0] === 254 && data[1] === 255 ? "utf-16be" : "utf-8";
		try {
			parser.write(new TextDecoder(encoding).decode(data)).close();
		} catch {
			continue;
		}
		for (const declared of partFamilies) for (const family of declared.split(";").map((value) => value.trim()).filter((value) => value && !/^\+(?:mj|mn)-(?:lt|ea|cs)$/.test(value))) families.set(normalize(family), family);
		for (const point of partPoints) codePoints.add(point);
	}
	return {
		families,
		codePoints: [...codePoints]
	};
}
//#endregion
//#region lib/types/document.js
/** Select bounded OOXML inspection or a legacy Office compound-file import. */
/** Input suffixes accepted by the public converter. */
const DOCUMENT_EXTENSIONS = [
	"doc",
	"docx",
	"odt",
	"xls",
	"xlsx",
	"ods",
	"ppt",
	"pptx",
	"odp"
];
const COMPOUND_SIGNATURE = [
	208,
	207,
	17,
	224,
	161,
	177,
	26,
	225
];
/**
* Inspect the input container before invoking LibreOffice's document importer.
* Compound-file streams and legacy font tables are interpreted by LibreOffice;
* they do not supply the API's OOXML font diagnostics.
* @param bytes - Source bytes already bounded by maxInputBytes.
* @param extension - Lowercase source suffix.
* @param limits - Limits applied to ZIP-based document contents.
* @returns OOXML font metadata, or empty diagnostics for a binary document.
*/
function inspectDocument(bytes, extension, limits) {
	if (!DOCUMENT_EXTENSIONS.includes(extension)) throw new ConversionError("unsupported-format", `Input extension must be ${DOCUMENT_EXTENSIONS.join(", ")}.`);
	if ([
		"docx",
		"xlsx",
		"pptx",
		"odt",
		"ods",
		"odp"
	].includes(extension)) return inspectDocument$1(bytes, extension, limits);
	if (bytes.length < 512 || !COMPOUND_SIGNATURE.every((value, index) => bytes[index] === value)) throw new ConversionError("invalid-document", "Legacy Office input must contain an OLE compound document.");
	const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const version = header.getUint16(26, true);
	const sectorShift = header.getUint16(30, true);
	if (header.getUint16(28, true) !== 65534 || header.getUint16(32, true) !== 6 || !(version === 3 && sectorShift === 9 || version === 4 && sectorShift === 12) || bytes.length < 2 ** sectorShift * 2 || bytes.length % 2 ** sectorShift !== 0) throw new ConversionError("invalid-document", "Legacy Office input has an invalid or truncated compound-file header.");
	return {
		families: /* @__PURE__ */ new Map(),
		codePoints: []
	};
}
//#endregion
//#region lib/types/font-loader.js
/** Conversion-local font imports backed by a factory-lifetime match cache. */
const faceKey = (face) => `${face.path}\0${face.faceIndex}`;
const requestKey = ({ codePoints: _codePoints, ...request }) => JSON.stringify(request);
/** Build one operation's loader over a factory-owned immutable font snapshot. */
function createFontLoader(options, document, install, faces, cache = []) {
	const catalog = new SystemFontCatalog({
		faces,
		fallbackFamilies: options.fontFallbacks
	});
	const byFace = new Map(faces.map((face) => [faceKey(face), face]));
	const matches = /* @__PURE__ */ new Map();
	for (const entry of cache.slice(-options.maxFontResolutionEntries)) matches.set(entry.key, {
		...entry,
		codePoints: [...entry.codePoints],
		faces: entry.faces.map((face) => ({ ...face }))
	});
	const loaded = /* @__PURE__ */ new Map();
	const resolved = /* @__PURE__ */ new Map();
	const missing = /* @__PURE__ */ new Map();
	const substitutions = /* @__PURE__ */ new Map();
	const signal = new AbortController().signal;
	let bytes = 0;
	function retain(entry) {
		matches.delete(entry.key);
		matches.set(entry.key, entry);
		while (matches.size > options.maxFontResolutionEntries) matches.delete(matches.keys().next().value);
	}
	function cachedMatch(request) {
		const key = requestKey(request);
		const points = [...new Set(request.codePoints)].sort((left, right) => left - right);
		const prior = matches.get(key);
		if (prior && (points.length === 0 && prior.emptyRequest || points.every((point) => prior.codePoints.includes(point)))) {
			retain(prior);
			return {
				fonts: prior.faces.map((reference) => byFace.get(faceKey(reference))).filter((face) => face !== void 0),
				...prior.missingFamily === void 0 ? {} : { missingFamily: prior.missingFamily }
			};
		}
		const unknown = prior === void 0 ? points : points.filter((point) => !prior.codePoints.includes(point));
		const current = catalog.match({
			...request,
			codePoints: points.length === 0 ? [] : unknown
		}, signal);
		const references = new Map((prior?.faces ?? []).map((face) => [faceKey(face), face]));
		for (const face of current.fonts) references.set(faceKey(face), {
			path: face.path,
			faceIndex: face.faceIndex
		});
		const missingFamily = prior?.missingFamily ?? current.missingFamily;
		const entry = {
			key,
			codePoints: [.../* @__PURE__ */ new Set([...prior?.codePoints ?? [], ...points])].sort((left, right) => left - right),
			emptyRequest: prior?.emptyRequest === true || points.length === 0,
			faces: [...references.values()],
			...missingFamily === void 0 ? {} : { missingFamily }
		};
		retain(entry);
		return {
			fonts: entry.faces.map((reference) => byFace.get(faceKey(reference))).filter((face) => face !== void 0),
			...entry.missingFamily === void 0 ? {} : { missingFamily: entry.missingFamily }
		};
	}
	return {
		get missingFonts() {
			return [...missing.values()];
		},
		get substitutions() {
			return [...substitutions.values()];
		},
		get files() {
			return [...loaded.values()];
		},
		get cacheEntries() {
			return [...matches.values()];
		},
		resolve(request) {
			const key = JSON.stringify(request);
			const cached = resolved.get(key);
			if (cached) return cached;
			const match = cachedMatch(request);
			if (match.missingFamily && document.families.has(normalize(match.missingFamily))) {
				const family = normalize(match.missingFamily);
				missing.set(family, match.missingFamily);
				if (match.fonts[0]) substitutions.set(family, {
					family: match.missingFamily,
					substitute: match.fonts[0].family
				});
			}
			const paths = match.fonts.map((face) => {
				const installed = loaded.get(face.path);
				if (installed !== void 0) return installed;
				if (bytes + face.size > options.maxLoadedFontBytes) throw new Error("Imported fonts exceed maxLoadedFontBytes.");
				const path = install(`${loaded.size}${extname(face.path)}`, readFont(face));
				bytes += face.size;
				loaded.set(face.path, path);
				return path;
			});
			resolved.set(key, paths);
			return paths;
		}
	};
}
/** Supply native imports and seed WASM's first usable default font. */
function preloadFonts(loader, options, document, native = false) {
	const families = [
		...options.initialFontFamilies,
		...native ? [...document.families.values()] : [],
		"sans-serif"
	];
	for (const family of families) loader.resolve({
		family,
		style: "",
		weight: 5,
		italic: 0,
		width: 5,
		pitch: 0,
		language: "",
		codePoints: native ? document.codePoints : []
	});
}
//#endregion
//#region lib/types/native-process.js
/** Native helper stdio; Windows pipe handles permit read/write access under restricted tokens. */
let pipeAttributes;
function windowsPipes() {
	const koffi = createRequire(import.meta.url)("koffi");
	const kernel = koffi.load("kernel32.dll"), advapi = koffi.load("advapi32.dll"), node = koffi.load(null);
	const attributes = pipeAttributes ??= koffi.struct({
		nLength: "uint32",
		lpSecurityDescriptor: "void *",
		bInheritHandle: "int"
	});
	const createPipe = kernel.func("__stdcall", "CreatePipe", "int", [
		koffi.out(koffi.pointer("void *")),
		koffi.out(koffi.pointer("void *")),
		koffi.pointer(attributes),
		"uint32"
	]);
	const closeHandle = kernel.func("int __stdcall CloseHandle(void *handle)");
	const localFree = kernel.func("void * __stdcall LocalFree(void *memory)");
	const peek = kernel.func("int __stdcall PeekNamedPipe(void *pipe, void *buffer, uint32 size, void *read, _Out_ uint32 *available, void *remaining)");
	const lastError = kernel.func("uint32 __stdcall GetLastError()");
	const convertSecurityDescriptor = advapi.func("int __stdcall ConvertStringSecurityDescriptorToSecurityDescriptorW(void *sddl, uint32 revision, _Out_ void **descriptor, void *size)");
	const openFd = node.func("int uv_open_osfhandle(void *handle)");
	function pipe() {
		const reads = [null], writes = [null];
		const descriptor = [null];
		if (!convertSecurityDescriptor(Buffer.from("D:(A;;GRGW;;;WD)\0", "utf16le"), 1, descriptor, null)) throw new Error(`Pipe security descriptor failed: ${lastError()}`);
		try {
			if (!createPipe(reads, writes, {
				nLength: attributes.size,
				lpSecurityDescriptor: descriptor[0],
				bInheritHandle: 0
			}, 0)) throw new Error(`CreatePipe failed: ${lastError()}`);
		} finally {
			localFree(descriptor[0]);
		}
		const read = openFd(reads[0]);
		if (read < 0) {
			closeHandle(reads[0]);
			closeHandle(writes[0]);
			throw new Error("Cannot open anonymous input descriptor");
		}
		const write = openFd(writes[0]);
		if (write < 0) {
			closeSync(read);
			closeHandle(writes[0]);
			throw new Error("Cannot open anonymous output descriptor");
		}
		let timer;
		let ended = false;
		const readable = new Readable({
			read() {
				if (timer === void 0 && !ended) poll();
			},
			destroy(error, callback) {
				ended = true;
				clearTimeout(timer);
				try {
					closeSync(read);
					callback(error);
				} catch (cause) {
					callback(error ?? cause);
				}
			}
		});
		function poll() {
			timer = void 0;
			try {
				const available = [0];
				if (!peek(reads[0], null, 0, null, available, null)) {
					const code = lastError();
					if (code !== 109) throw new Error(`PeekNamedPipe failed: ${code}`);
					ended = true;
					readable.push(null);
					return;
				}
				if (available[0] > 0) {
					const buffer = Buffer.allocUnsafe(Math.min(available[0], 65536));
					const count = readSync(read, buffer, 0, buffer.length, null);
					if (!readable.push(buffer.subarray(0, count))) return;
				}
				timer = setTimeout(poll, 5);
			} catch (error) {
				readable.destroy(error);
			}
		}
		return {
			read,
			write,
			readable
		};
	}
	return { pipe };
}
/**
* Spawn with inherited token and Job restrictions; pipe creation never requests a new sandbox policy.
* @param executable - Verified native helper executable.
* @param args - Helper protocol arguments.
* @param env - Filtered operation environment.
* @returns Helper streams and process lifetime, with output drained before close.
*/
function spawnNative(executable, args, env) {
	if (process.platform !== "win32") return spawn(executable, args, {
		stdio: "pipe",
		windowsHide: true,
		env
	});
	const { pipe } = windowsPipes();
	const owned = [];
	let child;
	try {
		const input = pipe();
		owned.push(input);
		const output = pipe();
		owned.push(output);
		const error = pipe();
		owned.push(error);
		child = spawn(executable, args, {
			stdio: [
				input.read,
				output.write,
				error.write
			],
			windowsHide: true,
			env
		});
	} catch (error) {
		for (const entry of owned) {
			entry.readable.destroy();
			closeSync(entry.write);
		}
		throw error;
	}
	const [input, output, error] = owned;
	input.readable.destroy();
	closeSync(output.write);
	closeSync(error.write);
	const stdin = createWriteStream("", {
		fd: input.write,
		autoClose: true
	});
	const helper = new class extends EventEmitter {
		stdin = stdin;
		stdout = output.readable;
		stderr = error.readable;
		get pid() {
			return child.pid;
		}
		get exitCode() {
			return child.exitCode;
		}
		get signalCode() {
			return child.signalCode;
		}
		kill(signal) {
			return child.kill(signal);
		}
	}();
	let failure;
	const fail = (cause) => {
		failure ??= cause;
		child.kill("SIGKILL");
	};
	stdin.on("error", fail);
	output.readable.on("error", fail);
	error.readable.on("error", fail);
	const drained = Promise.allSettled([
		finished(stdin),
		finished(output.readable),
		finished(error.readable)
	]);
	child.once("error", (cause) => {
		failure ??= cause;
	});
	child.once("close", (code, signal) => {
		stdin.destroy();
		output.readable.resume();
		error.readable.resume();
		drained.then(() => {
			if (failure !== void 0) helper.emit("error", failure);
			helper.emit("close", code, signal);
		});
	});
	return helper;
}
//#endregion
//#region lib/types/profile.js
/** Private headless profile used by both native and WASM conversions. */
/**
* CSV selection is checked by the engine wrapper before saving; suppress the interactive warning.
* @param fontNodes - Escaped VCL font-substitution nodes, or an empty string for WASM.
* @returns The per-operation registry XML; macro execution is independently disabled at load.
*/
function profileXml(fontNodes = "") {
	return `<?xml version="1.0" encoding="UTF-8"?><oor:items xmlns:oor="http://openoffice.org/2001/registry"><item oor:path="/org.openoffice.Office.Calc/Input"><prop oor:name="WarnActiveSheet" oor:op="fuse"><value>false</value></prop></item><item oor:path="/org.openoffice.VCL/FontSubstitutions/en">${fontNodes}</item></oor:items>\n`;
}
//#endregion
//#region lib/types/native-font-names.js
/** Localized search names from LibreOffice Core's fixed font-name dictionary. */
const LOCALIZED_FONT_SEARCH_NAMES = /* @__PURE__ */ new Map([
	["바탕", "batang"],
	["바탕체", "batangche"],
	["궁서", "gungshu"],
	["궁서체", "gungshuche"],
	["굴림", "gulim"],
	["굴림체", "gulimche"],
	["돋움", "dotum"],
	["돋움체", "dotumche"],
	["宋体", "simsun"],
	["新宋体", "nsimsun"],
	["黑体", "simhei"],
	["楷体", "simkai"],
	["中易宋体", "zycjksun"],
	["中易黑体", "zycjkhei"],
	["中易楷体", "zycjkkai"],
	["方正黑体", "fzhei"],
	["方正楷体", "fzkai"],
	["方正宋一", "fzsong"],
	["方正书宋", "fzshusong"],
	["方正仿宋", "fzfangsong"],
	["m简黑", "mhei"],
	["m潠窪", "mhei"],
	["m简楷噬", "mkai"],
	["m潠翱抎", "mkai"],
	["m简宋", "msong"],
	["m潠冼", "msong"],
	["m简夫宋", "cfangsong"],
	["m潠溘冼", "cfangsong"],
	["細明體", "mingliu"],
	["新細明體", "pmingliu"],
	["桥", "hei"],
	["歡", "kai"],
	["浩湧", "ming"],
	["msゴシック", "msgothic"],
	["mspゴシック", "mspgothic"],
	["ms明朝", "msmincho"],
	["msp明朝", "mspmincho"],
	["微軟正黑體", "microsoftjhenghei"],
	["微软雅黑", "microsoftyahei"],
	["メイリオ", "meiryo"],
	["hg明朝l", "hgminchol"],
	["hgゴシックb", "hggothicb"],
	["hgp明朝l", "hgpminchol"],
	["hgpゴシックb", "hgpgothicb"],
	["hg明朝lsun", "hgmincholsun"],
	["hgゴシックbsun", "hggothicbsun"],
	["hgp明朝lsun", "hgpmincholsun"],
	["hgpゴシックbsun", "hgpgothicbsun"],
	["hg平成明朝体", "hgheiseimin"],
	["hg平成明朝体w3x12", "hgheiseimin"],
	["ipa明朝", "ipamincho"],
	["ipap明朝", "ipapmincho"],
	["ipaゴシック", "ipagothic"],
	["ipapゴシック", "ipapgothic"],
	["ipauiゴシック", "ipauigothic"],
	["takao明朝", "takaomincho"],
	["takaop明朝", "takaopmincho"],
	["takaoゴシック", "takaogothic"],
	["takaopゴシック", "takaopgothic"],
	["さざなみ明朝", "sazanamimincho"],
	["さざなみゴシック", "sazanamigothic"],
	["東風明朝", "kochimincho"],
	["東風ゴシック", "kochigothic"],
	["썬돋움", "sundotum"],
	["썬굴림", "sungulim"],
	["썬바탕", "sunbatang"],
	["백묵돋움", "baekmukdotum"],
	["백묵굴림", "baekmukgulim"],
	["백묵바탕", "baekmukbatang"],
	["方正黑體", "fzheiti"],
	["方正楷體", "fzkaitib"],
	["方正明體", "fzmingtib"],
	["方正宋体", "fzsongti"],
	["hy견명조", "hymyeongjoextra"],
	["hy신명조", "hysinmyeongjomedium"],
	["hy중고딕", "hygothicmedium"],
	["hy그래픽m", "hygraphicmedium"],
	["hy그래픽", "hygraphic"],
	["새굴림", "newgulim"],
	["썬궁서", "sungungseo"],
	["hy궁서b", "hygungsobold"],
	["hy궁서", "hygungso"],
	["썬헤드라인", "sunheadline"],
	["hy헤드라인m", "hyheadlinemedium"],
	["hy헤드라인", "hyheadline"],
	["휴먼옛체", "yetr"],
	["hy견고딕", "hygothicextra"],
	["썬목판", "sunmokpan"],
	["썬엽서", "sunyeopseo"],
	["썬백송", "sunbaeksong"],
	["hy엽서l", "hypostlight"],
	["hy엽서", "hypost"],
	["휴먼매직체", "magicr"],
	["썬크리스탈", "suncrystal"],
	["썬샘물", "sunsaemmul"],
	["hy얕은샘물m", "hyshortsamulmedium"],
	["hy얕은샘물", "hyshortsamul"],
	["한컴바탕", "haansoftbatang"],
	["한컴돋움", "haansoftdotum"],
	["한양해서", "hyhaeseo"],
	["md솔체", "mdsol"],
	["md개성체", "mdgaesung"],
	["md아트체", "mdart"],
	["md아롱체", "mdalong"],
	["md이솝체", "mdeasop"],
	["hgｺﾞｼｯｸe", "hggothice"],
	["hgpｺﾞｼｯｸe", "hgpgothice"],
	["hgsｺﾞｼｯｸe", "hgsgothice"],
	["hgｺﾞｼｯｸm", "hggothicm"],
	["hgpｺﾞｼｯｸm", "hgpgothicm"],
	["hgsｺﾞｼｯｸm", "hgsgothicm"],
	["hg行書体", "hggyoshotai"],
	["hgp行書体", "hgpgyoshotai"],
	["hgs行書体", "hgsgyoshotai"],
	["hg教科書体", "hgkyokashotai"],
	["hgp教科書体", "hgpkyokashotai"],
	["hgs教科書体", "hgskyokashotai"],
	["hg明朝b", "hgminchob"],
	["hgp明朝b", "hgpminchob"],
	["hgs明朝b", "hgsminchob"],
	["hg明朝e", "hgminchoe"],
	["hgp明朝e", "hgpminchoe"],
	["hgs明朝e", "hgsminchoe"],
	["hg創英角ﾎﾟｯﾌﾟ体", "hgsoeikakupoptai"],
	["hgp創英角ﾎﾟｯﾌﾟ体", "hgpsoeikakupopta"],
	["hgs創英角ﾎﾟｯﾌﾟ体", "hgssoeikakupopta"],
	["hg創英ﾌﾟﾚｾﾞﾝｽeb", "hgsoeipresenceeb"],
	["hgp創英ﾌﾟﾚｾﾞﾝｽeb", "hgpsoeipresenceeb"],
	["hgs創英ﾌﾟﾚｾﾞﾝｽeb", "hgssoeipresenceeb"],
	["hg創英角ｺﾞｼｯｸub", "hgsoeikakugothicub"],
	["hgp創英角ｺﾞｼｯｸub", "hgpsoeikakugothicub"],
	["hgs創英角ｺﾞｼｯｸub", "hgssoeikakugothicub"],
	["hg正楷書体-pro", "hgseikaishotaipro"],
	["hg丸ｺﾞｼｯｸ-pro", "hgmarugothicmpro"],
	["ヒラギノ明朝pro", "hiraginominchopro"],
	["ヒラギノ明朝pron", "hiraginominchopron"],
	["ヒラギノ角ゴシック", "hiraginosans"],
	["ヒラギノ角ゴpro", "hiraginokakugothicpro"],
	["ヒラギノ角ゴpron", "hiraginokakugothicpron"],
	["ヒラギノ丸ゴpro", "hiraginomarugothicpro"],
	["ヒラギノ丸ゴpron", "hiraginomarugothicpron"],
	["游ゴシック", "yugothic"],
	["游ゴシック体", "yugothictai"],
	["游明朝", "yumincho"],
	["游明朝体", "yuminchotai"],
	["源ノ角ゴシック", "sourcehansans"],
	["源ノ角ゴシックjp", "sourcehansansjp"],
	["源ノ角ゴシックhw", "sourcehansanshw"],
	["源ノ明朝", "sourcehanserif"],
	["源ノ明朝jp", "sourcehanserifjp"],
	["ipamj明朝", "ipamjmincho"],
	["ipaexゴシック", "ipaexgothic"],
	["ipaex明朝", "ipaexmimcho"]
]);
//#endregion
//#region lib/types/native.js
const METRIC_SEARCH_NAMES = /* @__PURE__ */ new Map([
	["timesnewroman", "liberationserif"],
	["arial", "liberationsans"],
	["arialnarrow", "liberationsansnarrow"],
	["couriernew", "liberationmono"],
	["cambria", "caladea"],
	["calibri", "carlito"],
	["bundessans", "calibri"],
	["bundessansoffice", "calibri"],
	["bundessansregular", "calibri"],
	["bundesserif", "cambria"],
	["bundesserifoffice", "cambria"],
	["bundesserifregular", "cambria"]
]);
/** Environment variables the helper inherits from the embedding process. */
const NATIVE_ENVIRONMENT_KEYS = [
	"PATH",
	"SystemRoot",
	"SYSTEMROOT",
	"WINDIR",
	"COMSPEC",
	"PATHEXT",
	"LANG",
	"LC_ALL",
	"TZ",
	"OFFICE_GRAPHITE_DIAGNOSTICS",
	"OFFICE_LOAD_DIAGNOSTICS"
];
function fontSearchName(value) {
	const name = value.replaceAll(/[\uff00-\uff5e]/g, (character) => String.fromCharCode(character.charCodeAt(0) - 65248)).replaceAll(/[A-Z]/g, (character) => character.toLowerCase()).replaceAll(/[^a-z0-9;()\u0080-\uffff]/g, "");
	return LOCALIZED_FONT_SEARCH_NAMES.get(name) ?? name;
}
/**
* Write missing-family choices into the private profile's VCL substitution table.
* Installed originals and metric-compatible fonts are resolved before this table by LibreOffice.
* @param profile - Existing, conversion-owned profile directory.
* @param substitutions - Missing document families and the selected installed family.
* @returns Resolves after the exclusive profile file is written; empty choices still disable the interactive CSV warning.
*/
async function prepareNativeFontProfile(profile, substitutions) {
	const families = new Map(substitutions.map(({ family, substitute }) => [fontSearchName(family), substitute]));
	for (const [family, substitute] of [...families]) for (let alias = METRIC_SEARCH_NAMES.get(family); alias; alias = METRIC_SEARCH_NAMES.get(alias)) if (!families.has(alias)) families.set(alias, substitute);
	const escape = (value) => value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll("\"", "&quot;").replaceAll("'", "&apos;");
	const xml = profileXml([...families].map(([family, substitute]) => `<node oor:name="${escape(family)}" oor:op="replace"><prop oor:name="SubstFonts" oor:op="fuse"><value>${escape(substitute)}</value></prop></node>`).join(""));
	const user = join(profile, "user");
	await mkdir(user, { mode: 448 });
	await writeFile(join(user, "registrymodifications.xcu"), xml, {
		flag: "wx",
		mode: 384
	});
}
/**
* Native conversion and rendering disable printer discovery to avoid driver UI and network waits.
* Linux searches the verified program directory; ambient loader overrides and credentials are excluded.
* @param profile - Conversion-owned profile directory the helper treats as home.
* @param programDirectory - Verified engine program directory.
* @param source - Embedding process environment.
* @param platform - Host operating system.
* @returns the environment the helper starts with.
*/
function nativeEnvironment(profile, programDirectory, source = process.env, platform = process.platform) {
	const env = {};
	for (const key of NATIVE_ENVIRONMENT_KEYS) {
		const value = source[key];
		if (value !== void 0) env[key] = value;
	}
	if (platform === "linux") env.LD_LIBRARY_PATH = programDirectory;
	return Object.assign(env, {
		HOME: profile,
		USERPROFILE: profile,
		TMPDIR: profile,
		TMP: profile,
		TEMP: profile,
		SAL_DISABLE_PRINTERLIST: "1",
		SAL_DISABLE_DEFAULTPRINTER: "1"
	});
}
/**
* Prepare short Windows engine paths owned by the caller's private profile.
* @param engine - Verified engine installation; its resources remain in place.
* @param profile - Private operation directory that owns the temporary executable and junction.
* @returns Engine paths and a release function to call once, after the helper closes and before removing the profile.
*/
async function prepareNativeEngine(engine, profile) {
	if (process.platform !== "win32" || engine.executable.length < 260 && engine.programDirectory.length < 260) return {
		engine,
		release: async () => {}
	};
	const root = join(profile, "engine");
	await symlink(engine.root, root, "junction");
	try {
		const executable = join(profile, "dsoffice.exe");
		await copyFile(engine.executable, executable, constants.COPYFILE_EXCL);
		return {
			engine: {
				...engine,
				root,
				executable,
				programDirectory: join(root, relative(engine.root, engine.programDirectory))
			},
			release: () => unlink(root)
		};
	} catch (error) {
		await unlink(root);
		throw error;
	}
}
//#endregion
//#region lib/types/editor-protocol.js
/** Encode a control record; the native editor accepts no binary input. */
function encodeEditorRequest(control) {
	const json = Buffer.from(JSON.stringify(control));
	if (json.length > 1048576) throw new RangeError("Editor request exceeds 1 MiB.");
	const header = Buffer.alloc(8);
	header.writeUInt32BE(json.length);
	return Buffer.concat([header, json]);
}
/** Parses fragmented records without repeatedly copying a partial pixel frame. */
var EditorFrameReader = class {
	maxPixelBytes;
	receive;
	maxControlBytes;
	header = Buffer.alloc(8);
	headerBytes = 0;
	body;
	bodyBytes = 0;
	jsonBytes = 0;
	constructor(maxPixelBytes, receive, maxControlBytes = 1048576) {
		this.maxPixelBytes = maxPixelBytes;
		this.receive = receive;
		this.maxControlBytes = maxControlBytes;
	}
	/** Consume arbitrary process chunks; invalid sizes or JSON are fatal to the session. */
	push(chunk) {
		let offset = 0;
		while (offset < chunk.length) {
			if (this.body === void 0) {
				const count = Math.min(8 - this.headerBytes, chunk.length - offset);
				chunk.copy(this.header, this.headerBytes, offset, offset + count);
				offset += count;
				this.headerBytes += count;
				if (this.headerBytes !== 8) continue;
				this.jsonBytes = this.header.readUInt32BE(0);
				const pixels = this.header.readUInt32BE(4);
				if (this.jsonBytes < 1 || this.jsonBytes > this.maxControlBytes || pixels > this.maxPixelBytes) throw new RangeError("Native editor returned an invalid frame length.");
				this.body = Buffer.allocUnsafe(this.jsonBytes + pixels);
				this.bodyBytes = 0;
			}
			const count = Math.min(this.body.length - this.bodyBytes, chunk.length - offset);
			chunk.copy(this.body, this.bodyBytes, offset, offset + count);
			offset += count;
			this.bodyBytes += count;
			if (this.bodyBytes !== this.body.length) continue;
			const value = JSON.parse(this.body.toString("utf8", 0, this.jsonBytes));
			if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("Invalid editor control record.");
			const frame = {
				control: value,
				pixels: this.body.subarray(this.jsonBytes)
			};
			this.headerBytes = 0;
			this.body = void 0;
			this.receive(frame);
		}
	}
	/** Reject truncated records when stdout closes. */
	finish() {
		if (this.headerBytes !== 0 || this.body !== void 0) throw new Error("Native editor closed during a frame.");
	}
};
/** Read an integer from the native process, rejecting invalid counters and dimensions. */
function editorInteger(control, key, minimum = 0) {
	const value = control[key];
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum) throw new Error(`Invalid editor response field: ${key}.`);
	return value;
}
/** Validate straight RGBA and retain the reader-owned buffer without copying its pixels. */
function decodeEditorPixels({ control: c, pixels }) {
	const width = editorInteger(c, "pixelWidth", 1), height = editorInteger(c, "pixelHeight", 1);
	if (width > 4096 || height > 4096 || pixels.length !== width * height * 4 || c.pixelFormat !== "rgba-straight") throw new Error("Invalid native editor pixel result.");
	return {
		width,
		height,
		pixels: new Uint8Array(pixels.buffer, pixels.byteOffset, pixels.byteLength),
		revision: editorInteger(c, "revision"),
		...c.part === void 0 ? {} : { part: editorInteger(c, "part") },
		...c.ended === void 0 ? {} : { ended: c.ended === true }
	};
}
//#endregion
//#region lib/types/editor-transport.js
/** Owns one isolated native process and settles every request on cancellation, timeout, or exit. */
/** Process-private request multiplexer; the public editor serializes operations before submitting. */
var EditorTransport = class {
	timeoutMs;
	signal;
	onProgress;
	child;
	pending = /* @__PURE__ */ new Map();
	ready = Promise.withResolvers();
	exited = Promise.withResolvers();
	listeners = /* @__PURE__ */ new Set();
	failure;
	sequence = 0;
	stderr = "";
	closing = false;
	initialized = false;
	abort;
	startupTimer;
	constructor(executable, args, env, timeoutMs, maxPixelBytes, signal, maxControlBytes = 1048576, source, onProgress) {
		this.timeoutMs = timeoutMs;
		this.signal = signal;
		this.onProgress = onProgress;
		signal.throwIfAborted();
		this.child = spawnNative(executable, args, env);
		const reader = new EditorFrameReader(maxPixelBytes, (frame) => this.receive(frame), maxControlBytes);
		this.child.stdout.on("data", (chunk) => {
			try {
				reader.push(chunk);
			} catch (error) {
				this.fail(error instanceof Error ? error : new Error(String(error)));
			}
		});
		this.child.stderr.setEncoding("utf8");
		let diagnosticPending = "";
		this.child.stderr.on("data", (chunk) => {
			this.stderr = (this.stderr + chunk).slice(-65536);
			if (env.OFFICE_GRAPHITE_DIAGNOSTICS !== "1" && env.OFFICE_LOAD_DIAGNOSTICS !== "1") return;
			const lines = (diagnosticPending + chunk).split("\n");
			diagnosticPending = lines.pop().slice(-65536);
			for (const raw of lines) {
				const line = raw.replace(/\r$/, "");
				const graphite = env.OFFICE_GRAPHITE_DIAGNOSTICS === "1" && (line.startsWith("officeGraphiteFrame {") || /^office(?:Graphite|Ganesh) (?:[A-Za-z]+=\d+ ?)+$/.test(line));
				const loading = env.OFFICE_LOAD_DIAGNOSTICS === "1" && line.startsWith("officeLoad ");
				if (line.length <= 65536 && (graphite || loading)) process.stderr.write(`${line}\n`);
			}
		});
		this.child.stdin.on("error", (error) => this.fail(error));
		this.child.on("error", (error) => this.fail(error));
		this.child.once("close", (code) => {
			try {
				reader.finish();
			} catch (error) {
				this.fail(error instanceof Error ? error : new Error(String(error)));
			}
			if (!this.closing || this.pending.size > 0 || code !== 0) this.fail(/* @__PURE__ */ new Error(`Native editor exited (${code}). ${this.stderr}`));
			clearTimeout(this.startupTimer);
			this.signal.removeEventListener("abort", this.abort);
			this.exited.resolve();
		});
		this.abort = () => this.fail(signal.reason instanceof Error ? signal.reason : /* @__PURE__ */ new Error("Editor cancelled."));
		signal.addEventListener("abort", this.abort, { once: true });
		this.startupTimer = setTimeout(() => this.fail(/* @__PURE__ */ new Error("Native editor initialization timed out.")), timeoutMs);
		if (source) this.child.stdin.write(source, (error) => {
			if (error) this.fail(error);
		});
	}
	fail(error) {
		if (this.failure) return;
		this.failure = error;
		clearTimeout(this.startupTimer);
		this.ready.reject(error);
		for (const pending of this.pending.values()) {
			clearTimeout(pending.timer);
			pending.reject(error);
		}
		this.pending.clear();
		this.child.kill("SIGKILL");
		this.emit({
			kind: "failure",
			error
		});
	}
	emit(event) {
		for (const listener of this.listeners) try {
			listener(event);
		} catch (error) {
			process.emitWarning(error instanceof Error ? error : String(error));
		}
	}
	receive(frame) {
		const c = frame.control;
		if (c.kind === "loading") {
			if (c.stage !== "document" || frame.pixels.length !== 0 || this.initialized) throw new Error("Invalid native loading stage.");
			try {
				this.onProgress?.(c.stage);
			} catch (error) {
				process.emitWarning(error instanceof Error ? error : String(error));
			}
			return;
		}
		if (c.kind === "event" && Number.isSafeInteger(c.type) && typeof c.payload === "string" && frame.pixels.length === 0) {
			this.emit({
				kind: "callback",
				type: c.type,
				payload: c.payload
			});
			return;
		}
		if (c.kind === "revision" && Number.isSafeInteger(c.revision) && c.revision >= 0 && frame.pixels.length === 0) {
			this.emit({
				kind: "revision",
				revision: c.revision
			});
			return;
		}
		if (c.kind === "presentation" && Number.isSafeInteger(c.part) && c.part >= 0 && typeof c.ended === "boolean" && frame.pixels.length === 0) {
			this.emit({
				kind: "presentation",
				part: c.part,
				ended: c.ended
			});
			return;
		}
		if (c.kind === "ready") {
			if (c.protocol !== 4 || c.frameForwardingProtocol !== 1 || frame.pixels.length !== 0) throw new Error("Native editor requires editor protocol 4 and frame-forwarding protocol 1.");
			if (c.frameForwardingTransferable !== true) throw new Error("Native editor predates transferable frame-forwarding channels; install the matching engine.");
			this.initialized = true;
			clearTimeout(this.startupTimer);
			this.ready.resolve(frame);
			return;
		}
		if (c.kind !== "reply" || typeof c.id !== "number" || typeof c.ok !== "boolean") throw new Error("Invalid native editor reply.");
		const pending = this.pending.get(c.id);
		if (!pending) throw new Error("Native editor replied to an unknown request.");
		this.pending.delete(c.id);
		clearTimeout(pending.timer);
		if (c.ok) pending.resolve(frame);
		else pending.reject(new Error(typeof c.error === "string" ? c.error : "Native editor operation failed."));
	}
	/** Actual owned native process; never supplied by a renderer. */
	get processId() {
		if (this.child.pid === void 0) throw new Error("Native editor process has not started.");
		return this.child.pid;
	}
	/** Wait for document initialization; failure still requires close() to join cleanup. */
	opened() {
		return this.ready.promise;
	}
	/** Subscribe until disposed; observers must not throw. */
	onEvent(listener) {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}
	/** Submit a bounded operation; a timeout terminates the session because edit completion is then uncertain. */
	request(control) {
		if (this.failure) return Promise.reject(this.failure);
		if (this.closing) return Promise.reject(/* @__PURE__ */ new Error("Editor is closing."));
		const id = ++this.sequence;
		const bytes = encodeEditorRequest({
			...control,
			id
		});
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => this.fail(/* @__PURE__ */ new Error("Native editor operation timed out.")), this.timeoutMs);
			this.pending.set(id, {
				resolve,
				reject,
				timer
			});
			this.child.stdin.write(bytes, (error) => {
				if (error) this.fail(error);
			});
		});
	}
	/** Graceful EOF or forced termination; resolves only after the OS closes the process streams. */
	async close() {
		this.closing = true;
		this.child.stdin.end();
		const timer = setTimeout(() => this.child.kill("SIGKILL"), this.timeoutMs);
		try {
			await this.exited.promise;
		} finally {
			clearTimeout(timer);
			this.listeners.clear();
		}
	}
};
//#endregion
//#region lib/types/editor.js
/** Native, one-process-per-document editing. Batch conversion and Web WASM APIs are independent. */
function info({ control: c }) {
	const type = c.documentType;
	if (type !== 0 && type !== 1 && type !== 2) throw new Error("Unsupported native editor document type.");
	if (c.renderBackend !== "ganesh" && c.renderBackend !== "graphite" && c.renderBackend !== "raster") throw new Error("Invalid native editor rendering backend.");
	if (typeof c.rectangles !== "string" || !Array.isArray(c.parts) || c.parts.length < 1 || c.parts.length > 1e4) throw new Error("Invalid native editor document geometry.");
	const parts = c.parts.map((part) => {
		if (typeof part !== "object" || part === null || !("index" in part) || !("name" in part) || typeof part.index !== "number" || !Number.isSafeInteger(part.index) || part.index < 0 || typeof part.name !== "string") throw new Error("Invalid native editor part.");
		return {
			index: part.index,
			name: part.name
		};
	});
	return {
		documentType: type === 0 ? "text" : type === 1 ? "spreadsheet" : "presentation",
		width: editorInteger(c, "width", 1),
		height: editorInteger(c, "height", 1),
		part: editorInteger(c, "part"),
		rectangles: c.rectangles,
		parts,
		renderBackend: c.renderBackend
	};
}
function surfaceResult(frame) {
	const c = frame.control;
	if (frame.pixels.byteLength !== 0 || c.shared !== true || typeof c.requestId !== "string") throw new Error("Invalid native shared surface reply.");
	return {
		shared: true,
		requestId: c.requestId,
		width: editorInteger(c, "width", 1),
		height: editorInteger(c, "height", 1),
		revision: editorInteger(c, "revision"),
		...c.part === void 0 ? {} : { part: editorInteger(c, "part") },
		...c.ended === void 0 ? {} : { ended: c.ended === true }
	};
}
const snapshots = new FontSnapshotState();
const readCommands = /* @__PURE__ */ new Set([
	"SelectAll",
	"GoToStartOfDoc",
	"GoToEndOfDoc",
	"GoToCell",
	"Copy"
]);
/**
* Open an isolated native document from an owned, seekable memory stream, preparing its fonts.
* @param request - Validated-format bytes and explicit editing permission.
* @returns A live document; the caller must close it even after process failure.
* @throws If the matching native engine is missing, the input is invalid, or opening times out.
*/
async function openDocument(request) {
	if (![
		"docx",
		"xlsx",
		"pptx"
	].includes(request.format)) throw new TypeError("Native editing requires DOCX, XLSX, or PPTX.");
	const progress = (stage) => {
		try {
			request.onProgress?.(stage);
		} catch (error) {
			process.emitWarning(error instanceof Error ? error : String(error));
		}
	};
	const options = resolveOptions(request.options);
	if (!(request.bytes instanceof Uint8Array) || request.bytes.byteLength < 1 || request.bytes.byteLength > options.maxInputBytes) throw new RangeError("Editor input exceeds maxInputBytes or is empty.");
	const renderBackend = request.renderBackend ?? "auto";
	const graphiteCacheBytes = request.graphiteCacheBytes ?? 134217728;
	if (!Number.isSafeInteger(graphiteCacheBytes) || graphiteCacheBytes < 2) throw new RangeError("Graphite cache budget must be an integer of at least two bytes.");
	if (renderBackend !== "auto" && renderBackend !== "ganesh" && renderBackend !== "graphite" && renderBackend !== "raster") throw new TypeError("Unsupported native render backend.");
	const source = Buffer.from(request.bytes);
	const writable = request.writable === true;
	const lifetime = new AbortController();
	const signal = request.signal ? AbortSignal.any([request.signal, lifetime.signal]) : lifetime.signal;
	signal.throwIfAborted();
	const engine = await resolveEngine(request.options?.resolveEnginePackage);
	if (engine.backend !== "native" || engine.legacyInputPath) throw new Error("Native editing requires a preview-capable native engine package.");
	const scratch = await mkdtemp(join(tmpdir(), "libreoffice-editor-"));
	let transport;
	let prepared;
	try {
		progress("fonts");
		const metadata = inspectDocument(source, request.format, options);
		const profile = join(scratch, "profile"), fontsDirectory = join(scratch, "fonts");
		await Promise.all([mkdir(profile, { mode: 448 }), mkdir(fontsDirectory, { mode: 448 })]);
		const fonts = createFontLoader(options, metadata, (name, bytes) => {
			const path = join(fontsDirectory, name);
			writeFileSync(path, bytes, {
				flag: "wx",
				mode: 384
			});
			return path;
		}, (await snapshots.acquire(options, signal)).faces);
		preloadFonts(fonts, options, metadata, true);
		await prepareNativeFontProfile(profile, fonts.substitutions);
		progress("engine");
		prepared = await prepareNativeEngine(engine, profile);
		const selected = prepared.engine;
		transport = new EditorTransport(selected.executable, [
			"--operation",
			"edit",
			"--program-directory",
			selected.programDirectory,
			"--render-backend",
			renderBackend,
			"--graphite-cache-bytes",
			String(graphiteCacheBytes),
			"--input-bytes",
			String(source.byteLength),
			"--input-format",
			request.format,
			"--scratch-directory",
			scratch,
			"--profile-directory",
			profile,
			"--writable",
			String(writable),
			"--max-output-bytes",
			String(options.maxOutputBytes),
			"--max-image-resolution",
			String(options.maxImageResolution),
			...fonts.files.flatMap((path) => ["--font-file", path])
		], nativeEnvironment(profile, selected.programDirectory), options.timeoutMs, Math.min(options.maxOutputBytes, 67108864), signal, Math.max(1048576, options.maxOutputBytes), source, progress);
		const process = transport;
		const initialInfo = info(await process.opened());
		let revision = 0, closing = false, disposal, queuedBytes = 0;
		let tail = Promise.resolve();
		const listeners = /* @__PURE__ */ new Set();
		const emit = (event) => {
			for (const listener of listeners) try {
				listener(event);
			} catch (error) {
				globalThis.process.emitWarning(error instanceof Error ? error : String(error));
			}
		};
		const observeRevision = (next) => {
			if (next <= revision) return;
			revision = next;
			emit({
				kind: "revision",
				revision
			});
		};
		process.onEvent((event) => {
			if (event.kind === "revision") observeRevision(event.revision);
			else emit(event);
		});
		const call = async (control) => {
			const frame = await process.request(control);
			observeRevision(editorInteger(frame.control, "revision"));
			return frame;
		};
		const enqueue = (size, operation) => {
			if (closing) return Promise.reject(/* @__PURE__ */ new Error("Editor is closing."));
			if (queuedBytes + size > 8388608) return Promise.reject(/* @__PURE__ */ new Error("Editor input queue is full."));
			queuedBytes += size;
			const result = tail.then(operation).finally(() => {
				queuedBytes -= size;
			});
			tail = result.catch(() => {});
			return result;
		};
		const execute = (control) => enqueue(Buffer.byteLength(JSON.stringify(control)), () => call(control));
		const requireWritable = () => {
			if (!writable) throw new Error("Document is read-only.");
		};
		return {
			initialInfo,
			missingFonts: fonts.missingFonts,
			writable,
			processId: process.processId,
			async attachSurfaceTransport(tx, { timeoutMs }) {
				if (typeof tx !== "string" || tx.length === 0) throw new TypeError("A frame-forwarding TX credential is required.");
				if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs >= options.timeoutMs) throw new RangeError("Surface transport timeoutMs must be a positive integer below the operation timeout.");
				await execute({
					op: "attachSurfaceTransport",
					tx,
					timeoutMs
				});
			},
			async detachSurfaceTransport() {
				await execute({ op: "detachSurfaceTransport" });
			},
			async waitForInput(sequence) {
				await execute({
					op: "inputBarrier",
					sequence
				});
			},
			async renderSurface(rectangle, pixelWidth, pixelHeight, surface) {
				return surfaceResult(await execute({
					...rectangle,
					op: "paintSurface",
					pixelWidth,
					pixelHeight,
					surface
				}));
			},
			async presentationSurface(surface) {
				return surfaceResult(await execute({
					op: "presentationSurface",
					surface
				}));
			},
			get revision() {
				return revision;
			},
			onEvent(listener) {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
			async getInfo() {
				return info(await execute({ op: "info" }));
			},
			async dispatch(command, arguments_ = {}) {
				if (!readCommands.has(command)) requireWritable();
				await execute({
					op: "command",
					command: `.uno:${command}`,
					arguments: JSON.stringify(arguments_)
				});
			},
			async input(event) {
				const navigation = event.op === "key" && event.character === 0 && [
					1024,
					1025,
					1026,
					1027,
					1028,
					1029,
					1030,
					1031,
					1281,
					1282
				].includes(event.key & 4095);
				if (event.op !== "mouse" && !navigation) requireWritable();
				await execute(event);
			},
			async setViewport(viewport) {
				await execute({
					...viewport,
					op: "viewport"
				});
			},
			async setPart(part) {
				return info(await execute({
					op: "part",
					part
				}));
			},
			async render(rectangle, pixelWidth, pixelHeight) {
				return decodeEditorPixels(await execute({
					...rectangle,
					op: "paint",
					pixelWidth,
					pixelHeight
				}));
			},
			async getSelection(mime) {
				const frame = await execute({
					op: "selection",
					mime
				});
				if (typeof frame.control.text !== "string") throw new Error("Invalid native selection result.");
				return frame.control.text;
			},
			async getValues(command, area) {
				const frame = await execute({
					...area,
					op: "values",
					command: `.uno:${command}`
				});
				if (typeof frame.control.value !== "string") throw new Error("Invalid native command result.");
				return frame.control.value;
			},
			async presentation(control) {
				await execute({
					...control,
					op: "presentation"
				});
			},
			async presentationFrame() {
				return decodeEditorPixels(await execute({
					op: "presentation",
					action: "frame"
				}));
			},
			async exportSnapshot() {
				requireWritable();
				return enqueue(0, async () => {
					const path = join(scratch, `${randomUUID()}.${request.format}`);
					try {
						const frame = await call({
							op: "save",
							path,
							url: pathToFileURL(path).href,
							format: request.format
						});
						const bytes = await readFile(path);
						if (bytes.length > options.maxOutputBytes) throw new Error("Editor snapshot exceeds maxOutputBytes.");
						inspectDocument(bytes, request.format, options);
						return {
							bytes,
							format: request.format,
							revision: editorInteger(frame.control, "revision")
						};
					} finally {
						await rm(path, { force: true });
					}
				});
			},
			close() {
				disposal ??= (async () => {
					closing = true;
					await tail;
					try {
						await process.close();
					} finally {
						lifetime.abort();
						listeners.clear();
						try {
							await prepared?.release();
						} finally {
							await rm(scratch, {
								recursive: true,
								force: true
							});
						}
					}
				})();
				return disposal;
			}
		};
	} catch (error) {
		lifetime.abort(error);
		try {
			await transport?.close();
		} finally {
			try {
				await prepared?.release();
			} finally {
				await rm(scratch, {
					recursive: true,
					force: true
				});
			}
		}
		throw error;
	}
}
//#endregion
export { openDocument };
