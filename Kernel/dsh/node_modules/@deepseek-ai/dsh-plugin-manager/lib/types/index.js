var __runInitializers = (this && this.__runInitializers) || function (thisArg, initializers, value) {
    var useValue = arguments.length > 2;
    for (var i = 0; i < initializers.length; i++) {
        value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
    }
    return useValue ? value : void 0;
};
var __esDecorate = (this && this.__esDecorate) || function (ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
    function accept(f) { if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected"); return f; }
    var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
    var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
    var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
    var _, done = false;
    for (var i = decorators.length - 1; i >= 0; i--) {
        var context = {};
        for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
        for (var p in contextIn.access) context.access[p] = contextIn.access[p];
        context.addInitializer = function (f) { if (done) throw new TypeError("Cannot add initializers after decoration has completed"); extraInitializers.push(accept(f || null)); };
        var result = (0, decorators[i])(kind === "accessor" ? { get: descriptor.get, set: descriptor.set } : descriptor[key], context);
        if (kind === "accessor") {
            if (result === void 0) continue;
            if (result === null || typeof result !== "object") throw new TypeError("Object expected");
            if (_ = accept(result.get)) descriptor.get = _;
            if (_ = accept(result.set)) descriptor.set = _;
            if (_ = accept(result.init)) initializers.unshift(_);
        }
        else if (_ = accept(result)) {
            if (kind === "field") initializers.unshift(_);
            else descriptor[key] = _;
        }
    }
    if (target) Object.defineProperty(target, contextIn.name, descriptor);
    done = true;
};
/** Current-profile plugin and bundle management over shared dsh plugin operations. */
import { randomUUID } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { withFileLock, writeFileAtomic } from '@deepseek-ai/dsh-atomic-write';
import z from '@deepseek-ai/schemastery';
import semver from 'semver';
import { TypertRemoteService, Remote, RemoteError, remoteErrorOf } from '@deepseek-ai/dsh-typert-protocol';
import { detailImageResponse, NPM_FILE_SOURCES } from '@deepseek-ai/dsh-plugin-detail';
import { MAX_TIMER_DELAY_MS } from '@deepseek-ai/dsh-timeout';
import { readBundleDocument, readBundleImage } from "./bundle-detail.js";
import { pluginEntryId, readPluginInventory } from '@deepseek-ai/dsh-host-plugin-inventory';
import { readPluginMeta, readProfileManifest, resolveBundleDir, loadOverlayPatches, composeEntries, readProfilePatches, bundlePatchPaths, evaluatePluginCompatibility, readProfileCompatibility, readProfileVersionExemptions, setProfileVersionExemption, validatePluginVersionExemption, PROFILE_COMPATIBILITY_FILENAME, getDshRuntimeVersion, ResolutionRestartRequired, assertBundlePlatform, UnsupportedBundlePlatformError, } from '@deepseek-ai/dsh-app-boot';
import { OPTIONAL_BUNDLES, OFFICIAL_ON_DEMAND_CATALOG, PROTECTED_PLUGINS } from '@deepseek-ai/dsh-official-bundles';
import { bundleManifest, readProfileRegistry, registryArguments, runProfilePnpm, saveManifest, viewProfilePackage } from "./operations.js";
import { classifyInstallFailure } from "./install-failure.js";
import { officialBundleInstallTarget } from "./official-install-target.js";
import { dependencySpec, InvalidInstallSpecError, parseInstallSpec } from "./install-spec.js";
import { attributeFailure, normalizeRegistry, NPMMIRROR_REGISTRY, registryPlan } from "./registry.js";
import { flatten, writePluginEnabled } from "./patch.js";
import { incompatiblePlugin, ManagementFailure } from "./failure.js";
import { approveBuilds, recordPendingBuilds } from "./build-approval.js";
import { checkGithubConnection } from "./github-connection.js";
import { chooseGitTransport, sshGitSpec } from "./git-transport.js";
import { newerVersions, VERSION_FIELDS, VERSION_LIST_FIELDS, versionFacts } from "./versions.js";
export { classifyInstallFailure } from "./install-failure.js";
export { InvalidInstallSpecError, parseInstallSpec } from "./install-spec.js";
/** The Fetch route serving images beside bundle detail documents. */
const DETAIL_FILE_PATH = '/api/plugin-manager/detail-file';
/** An http(s) URL, as pnpm's `--registry` takes it. */
const REGISTRY_URL = /^https?:\/\/\S+$/;
/** The profile files an installation writes and a failed or cancelled one restores. */
const RESTORED_FILES = ['package.json', 'pnpm-lock.yaml'];
/** The manifest fields an inspection reads for a registry spec. */
const INSPECT_FIELDS = ['name', 'version', 'description', 'dsh'];
/** pnpm's colour escapes, which a JSON answer may be wrapped in. */
const ANSI_SEQUENCE = /\x1b\[[0-9;]*m/g;
/** Preserve the exact observed diagnostic, including non-Error failures. */
function messageOf(error) { return error instanceof Error ? error.message : String(error); }
/** An expected refusal keeps its code; anything else becomes an operation error carrying its exact diagnostic. */
function managementError(error) {
    if (error instanceof UnsupportedBundlePlatformError)
        return { code: 'unsupported-platform' };
    if (!(error instanceof ManagementFailure))
        return { code: 'operation-error', diagnostic: messageOf(error) };
    return { code: error.code, ...error.incompatible === undefined ? {} : { incompatible: error.incompatible } };
}
/** The caller stopped an installation; its files are restored before this is thrown. */
class InstallCancelledError extends Error {
    constructor() {
        super('Installation cancelled');
        this.name = 'InstallCancelledError';
    }
}
/** A manifest field that is a string, when the manifest carries one. */
function stringField(manifest, field) {
    const value = manifest[field];
    return typeof value === 'string' ? value : undefined;
}
/** What a package manifest says about the package: identity, one-liner, and whether it is a bundle. */
function inspectionOf(kind, manifest, registry) {
    const dsh = manifest.dsh;
    const declared = typeof dsh === 'object' && dsh !== null ? dsh : undefined;
    const bundle = declared !== undefined && typeof declared.bundle === 'object' && declared.bundle !== null;
    const name = stringField(manifest, 'name');
    const version = stringField(manifest, 'version');
    const description = stringField(manifest, 'description');
    return {
        status: 'accepted', kind, bundle, registry,
        ...name === undefined ? {} : { name },
        ...version === undefined ? {} : { version },
        ...description === undefined || description === '' ? {} : { description },
    };
}
function refused(problem, reason) {
    return { status: 'refused', problem, reason };
}
/**
 * Why an update cannot replace a listed bundle, or what it replaces.
 * @param bundle The listed bundle, or undefined for a name the inventory does not list.
 * @param name The package name, for the refusal.
 * @returns The bundle, its installed version, and its registry spec; or the refusal's diagnostic.
 */
function updateRefusal(bundle, name) {
    if (bundle === undefined || !bundle.installed || bundle.availability !== 'profile' || bundle.official || bundle.source === undefined) {
        return `${name} is not a third-party package installed in this profile`;
    }
    // A management bundle cannot be removed, so it cannot be replaced either.
    if (bundle.readOnlyReason !== undefined)
        return `${bundle.name} carries plugin management and cannot be removed`;
    const installed = bundle.version;
    if (installed === undefined || semver.valid(installed) === null)
        return `${bundle.name} declares no semver version`;
    const parsed = parsedForRegistry(bundle.source);
    // A protocol in the saved range, such as an `npm:` alias, names another package than the dependency.
    if (parsed.kind !== 'registry' || parsed.name !== bundle.name || parsed.range?.includes(':') === true) {
        return `${bundle.name} is not installed from a registry under its own name`;
    }
    return { bundle, installed, parsed };
}
/**
 * Map items with at most `limit` calls in flight, stopping new calls once one answers `stop`.
 * @param items - the items, in order.
 * @param limit - the most calls in flight.
 * @param map - the call for one item.
 * @param stop - whether an answer ends the remaining calls.
 * @returns the answers of the calls that ran, in item order; items after a stop stay absent.
 */
async function mapConcurrent(items, limit, map, stop) {
    const answers = items.map(() => undefined);
    let next = 0;
    let stopped = false;
    await Promise.all(Array.from({ length: Math.min(limit, items.length) }, async () => {
        while (!stopped && next < items.length) {
            const index = next++;
            const answer = await map(items[index]);
            answers[index] = answer;
            if (stop(answer))
                stopped = true;
        }
    }));
    return answers;
}
/** The refusal `pnpm view --json` prints on stdout, `{ error: { code, message } }`, as one log line; empty for anything else. */
function printedError(printed) {
    let parsed;
    try {
        parsed = JSON.parse(printed || 'null');
    }
    catch {
        return ''; /* not JSON: nothing pnpm printed as a refusal */
    }
    const error = typeof parsed === 'object' && parsed !== null ? parsed.error : undefined;
    if (typeof error !== 'object' || error === null)
        return '';
    const { code, message } = error;
    return [code, message].filter((part) => typeof part === 'string').join('  ');
}
/** The bundles a profile manifest selects. */
function bundleSelection(manifest) {
    return manifest.dsh?.profile?.bundles ?? [];
}
/**
 * The package an installation spec names before pnpm runs: a registry name, a local directory's manifest name, or the
 * dependency saved with this git or tarball spec or the SSH form of its Git URL, compared as the plugin list shows saved specs.
 * @param spec The installation spec.
 * @param dependencies The profile's saved dependencies.
 * @param profileDir The profile directory saved local paths are relative to.
 * @returns The package name, or `undefined` when only the installation tells it.
 */
function specPackageName(spec, dependencies, profileDir) {
    const parsed = parsedForRegistry(spec);
    if (parsed.kind === 'registry')
        return parsed.name;
    if (parsed.kind === 'path')
        return packageName(parsed.path);
    const alternative = parsed.kind === 'tarball' && parsed.path !== undefined ? `file:${parsed.path}` : sshGitSpec(parsed);
    const specs = alternative === undefined ? [spec] : [spec, alternative];
    return Object.entries(dependencies).find(([dependency, saved]) => specs.includes(dependencySpec(dependency, saved, profileDir)))?.[0];
}
/** The package name a local directory declares, or `undefined` without a readable manifest. */
function packageName(dir) {
    try {
        return readProfileManifest('dsh', dir).name;
    }
    catch (_error) {
        // pnpm reports the unreadable directory itself.
        return undefined;
    }
}
/** The spec's form, for deciding whether a failed attempt was the registry's; a form the parser refuses has no host of its own. */
function parsedForRegistry(spec) {
    try {
        return parseInstallSpec(spec);
    }
    catch (error) {
        /* v8 ignore next -- parseInstallSpec throws nothing but its own refusal */
        if (!(error instanceof InvalidInstallSpecError))
            throw error;
        return { kind: 'registry', spec, name: spec };
    }
}
/** Only registry package names are retained before installation resolves its target. */
function installOperationTarget(spec) {
    let parsed;
    try {
        parsed = parseInstallSpec(spec);
    }
    catch (_error) {
        return null; /* Invalid specs remain private; installation owns their refusal. */
    }
    return parsed.kind === 'registry' ? parsed.name : null;
}
/** Exemption facts use canonical package identity, never the raw request or installed version. */
function exemptionOperationIdentity(packageVersion, runtimeVersion) {
    try {
        validatePluginVersionExemption(packageVersion, runtimeVersion);
    }
    catch (_error) {
        return { target: null }; /* The operation still owns validation failure reporting. */
    }
    const separator = packageVersion.lastIndexOf('@');
    return { target: installOperationTarget(packageVersion.slice(0, separator)),
        version: diagnosticVersion(packageVersion.slice(separator + 1)) };
}
/** Unusable file metadata must not discard the rest of an operation record. */
function diagnosticVersion(value) {
    return typeof value === 'string' && value.length <= 128 ? value : undefined;
}
/** Manage profile files and apply their declared reload lifecycle. */
let PluginManager = (() => {
    let _classSuper = TypertRemoteService;
    let _instanceExtraInitializers = [];
    let _readBundleDetail_decorators;
    let _listVersionExemptions_decorators;
    let _setVersionExemption_decorators;
    let _listPlugins_decorators;
    let _listBundles_decorators;
    let _registries_decorators;
    let _inspect_decorators;
    let _listVersions_decorators;
    let _updateBundle_decorators;
    let _setPluginEnabled_decorators;
    let _setBundleEnabled_decorators;
    let _installBundle_decorators;
    let _waitForInstall_decorators;
    let _cancelInstall_decorators;
    let _removeBundle_decorators;
    return class PluginManager extends _classSuper {
        static {
            const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
            _readBundleDetail_decorators = [Remote];
            _listVersionExemptions_decorators = [Remote];
            _setVersionExemption_decorators = [Remote];
            _listPlugins_decorators = [Remote];
            _listBundles_decorators = [Remote];
            _registries_decorators = [Remote];
            _inspect_decorators = [Remote];
            _listVersions_decorators = [Remote];
            _updateBundle_decorators = [Remote];
            _setPluginEnabled_decorators = [Remote];
            _setBundleEnabled_decorators = [Remote];
            _installBundle_decorators = [Remote];
            _waitForInstall_decorators = [Remote];
            _cancelInstall_decorators = [Remote];
            _removeBundle_decorators = [Remote];
            __esDecorate(this, null, _readBundleDetail_decorators, { kind: "method", name: "readBundleDetail", static: false, private: false, access: { has: obj => "readBundleDetail" in obj, get: obj => obj.readBundleDetail }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _listVersionExemptions_decorators, { kind: "method", name: "listVersionExemptions", static: false, private: false, access: { has: obj => "listVersionExemptions" in obj, get: obj => obj.listVersionExemptions }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _setVersionExemption_decorators, { kind: "method", name: "setVersionExemption", static: false, private: false, access: { has: obj => "setVersionExemption" in obj, get: obj => obj.setVersionExemption }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _listPlugins_decorators, { kind: "method", name: "listPlugins", static: false, private: false, access: { has: obj => "listPlugins" in obj, get: obj => obj.listPlugins }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _listBundles_decorators, { kind: "method", name: "listBundles", static: false, private: false, access: { has: obj => "listBundles" in obj, get: obj => obj.listBundles }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _registries_decorators, { kind: "method", name: "registries", static: false, private: false, access: { has: obj => "registries" in obj, get: obj => obj.registries }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _inspect_decorators, { kind: "method", name: "inspect", static: false, private: false, access: { has: obj => "inspect" in obj, get: obj => obj.inspect }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _listVersions_decorators, { kind: "method", name: "listVersions", static: false, private: false, access: { has: obj => "listVersions" in obj, get: obj => obj.listVersions }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _updateBundle_decorators, { kind: "method", name: "updateBundle", static: false, private: false, access: { has: obj => "updateBundle" in obj, get: obj => obj.updateBundle }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _setPluginEnabled_decorators, { kind: "method", name: "setPluginEnabled", static: false, private: false, access: { has: obj => "setPluginEnabled" in obj, get: obj => obj.setPluginEnabled }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _setBundleEnabled_decorators, { kind: "method", name: "setBundleEnabled", static: false, private: false, access: { has: obj => "setBundleEnabled" in obj, get: obj => obj.setBundleEnabled }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _installBundle_decorators, { kind: "method", name: "installBundle", static: false, private: false, access: { has: obj => "installBundle" in obj, get: obj => obj.installBundle }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _waitForInstall_decorators, { kind: "method", name: "waitForInstall", static: false, private: false, access: { has: obj => "waitForInstall" in obj, get: obj => obj.waitForInstall }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _cancelInstall_decorators, { kind: "method", name: "cancelInstall", static: false, private: false, access: { has: obj => "cancelInstall" in obj, get: obj => obj.cancelInstall }, metadata: _metadata }, null, _instanceExtraInitializers);
            __esDecorate(this, null, _removeBundle_decorators, { kind: "method", name: "removeBundle", static: false, private: false, access: { has: obj => "removeBundle" in obj, get: obj => obj.removeBundle }, metadata: _metadata }, null, _instanceExtraInitializers);
            if (_metadata) Object.defineProperty(this, Symbol.metadata, { enumerable: true, configurable: true, writable: true, value: _metadata });
        }
        static inject = ['loader', 'profileContext'];
        static Config = z.object({
            pnpmCommand: z.string().default('pnpm'),
            outputBytes: z.number().step(1).min(1).default(16384),
            lockWaitMs: z.number().step(1).min(0).default(120000),
            inspectTimeoutMs: z.number().step(1).min(1000).default(20000),
            githubConnectionTimeoutMs: z.number().step(1).min(1000).default(5000),
            gitTransportTimeoutMs: z.number().step(1).min(1000).default(5000),
            idleTimeoutMs: z.number().step(1).min(1000).default(600000),
            registry: z.string().pattern(REGISTRY_URL),
            fallbackRegistries: z.array(z.string().pattern(REGISTRY_URL)).default([NPMMIRROR_REGISTRY]),
            versionListLimit: z.number().step(1).min(1).default(20),
            versionLookupConcurrency: z.number().step(1).min(1).default(4),
            detailMaxBytes: z.natural().min(1).default(1024 * 1024),
            detailImageMaxBytes: z.natural().min(1).default(5 * 1024 * 1024),
            detailFetchTimeoutMs: z.natural().min(1).max(MAX_TIMER_DELAY_MS).default(15000),
            detailNpmFileSources: z.array(z.union(NPM_FILE_SOURCES)).min(1).default([...NPM_FILE_SOURCES]),
        });
        /** Management bundles remain protected if their files become unreadable. */
        managementBundles = (__runInitializers(this, _instanceExtraInitializers), new Set());
        /**
         * The registry each package was last installed from by this manager in this process. pnpm verifies lockfile tarball
         * URLs against its default registry unless they belong to a named registry, so a later removal names it again.
         */
        installedFrom = new Map();
        ownerEntryId;
        packageOperations = new Set();
        profile;
        outputBytes;
        lockWaitMs;
        inspectTimeoutMs;
        githubConnectionTimeoutMs;
        gitTransportTimeoutMs;
        idleTimeoutMs;
        pnpmCommand;
        versionListLimit;
        versionLookupConcurrency;
        configuredRegistries;
        ownerContext;
        abort = new AbortController();
        /** Installations by request id, from their call until it settles. */
        installs = new Map();
        detail;
        constructor(ctx, config) {
            super(ctx, 'pluginManager');
            this.ownerEntryId = ctx.fiber.entry?.id;
            this.ownerContext = ctx;
            this.profile = ctx.profileContext;
            for (const name of this.profile.startedBundles)
                this.protectsManager(name);
            this.outputBytes = config.outputBytes;
            this.lockWaitMs = config.lockWaitMs;
            this.inspectTimeoutMs = config.inspectTimeoutMs;
            this.githubConnectionTimeoutMs = config.githubConnectionTimeoutMs;
            this.gitTransportTimeoutMs = config.gitTransportTimeoutMs;
            this.idleTimeoutMs = config.idleTimeoutMs;
            this.pnpmCommand = config.pnpmCommand;
            this.versionListLimit = config.versionListLimit;
            this.versionLookupConcurrency = config.versionLookupConcurrency;
            this.configuredRegistries = {
                registry: config.registry === undefined ? null : normalizeRegistry(config.registry),
                fallbackRegistries: config.fallbackRegistries.map(normalizeRegistry),
            };
            const required = config;
            this.detail = {
                profile: this.profile, maxDocumentBytes: required.detailMaxBytes, maxImageBytes: required.detailImageMaxBytes,
                npmFileSources: required.detailNpmFileSources, timeoutMs: required.detailFetchTimeoutMs, lifetime: this.abort.signal,
            };
            ctx.effect(() => async () => {
                this.abort.abort();
                await Promise.allSettled([...this.packageOperations]);
            }, 'plugin-manager: package cancellation');
            ctx.inject(['connection'], (scope) => {
                scope.effect(() => scope.connection.fetch.register({
                    path: DETAIL_FILE_PATH, methods: ['GET'], requestBody: 'buffered', fetch: request => this.serveDetailFile(request),
                }), `plugin-manager: ${DETAIL_FILE_PATH}`);
            });
        }
        /**
         * Read a bundle's long description: the Markdown file its locale `meta.detail` names in `language`, otherwise its
         * README. An installed bundle is read from its directory without network access; an official bundle the installation
         * offers before it is installed is read from the npm file CDNs at its offered version. Nothing is cached.
         * @param name - the bundle's package name, as `listBundles()` lists it.
         * @param language - the display language, such as `zh-CN`; documents fall back through its prefixes to English.
         * @param options - `npmFileSources` names npm file CDNs to ask first for an offered bundle, in order, such as the one
         *   a Client measured as fastest; configured CDNs it omits follow, and CDNs that are not configured are not asked.
         * @returns the document with its origin and path, or `none`.
         * @throws {RemoteError} `plugin-manager/unknown-bundle`, `plugin-manager/invalid-language`, or
         *   `plugin-manager/detail-unavailable`.
         */
        async readBundleDetail(name, language, options) {
            return await readBundleDocument(await this.listedBundle(name), language, options?.npmFileSources ?? [], this.detail);
        }
        async listedBundle(name) {
            const bundle = (await this.listBundles()).find(candidate => candidate.name === name);
            if (bundle === undefined)
                throw new RemoteError('plugin-manager/unknown-bundle', `No such bundle: ${name}`, { name });
            return bundle;
        }
        /**
         * Answer `GET ${DETAIL_FILE_PATH}?bundle=<name>&document=<path>&path=<destination>[&sources=<cdn>,...]` with one image
         * beside a bundle document; `sources` names npm file CDNs to ask first for an offered bundle, as `readBundleDetail` does.
         */
        async serveDetailFile(request) {
            const params = new URL(request.url).searchParams;
            const [bundle, document, path] = [params.get('bundle'), params.get('document'), params.get('path')];
            // Images are read again on every request, so a reinstalled bundle shows its own.
            const cache = 'no-store';
            if (bundle === null || document === null || path === null)
                return detailImageResponse('invalid', cache);
            try {
                const image = await readBundleImage(await this.listedBundle(bundle), document, path, params.get('sources')?.split(',') ?? [], this.detail);
                return detailImageResponse(image, cache);
            }
            catch (error) {
                const failure = remoteErrorOf(error);
                if (failure?.code === 'plugin-manager/unknown-bundle')
                    return detailImageResponse(undefined, cache);
                if (failure?.code === 'plugin-manager/detail-unavailable')
                    return detailImageResponse(failure.details.reason, cache);
                throw error;
            }
        }
        /** Read exact plugin-version exemptions saved in this profile.
         * @returns Accepted package-name@version keys with the runtime versions they may run on, and any
         * record or file problem the reader rejected, which the caller reports instead of failing.
         */
        listVersionExemptions() {
            const { exemptions, warnings } = readProfileCompatibility(this.profile.dir);
            return { exemptions, warnings };
        }
        /** Grant or revoke one exact plugin/runtime exemption and reevaluate live plugins.
         * @param packageVersion Exact manifest package name followed by @ and its version; never an installation spec or alias.
         * @param runtimeVersion Exact current DSH version for grants; revocation may name a previous runtime.
         * @param enabled Whether to grant rather than revoke the exemption.
         * @param acceptRisk Required true for grants after the user accepts possible crashes and data loss.
         * @returns Saved and runtime outcomes. Startup-only profiles require restart.
         */
        setVersionExemption(packageVersion, runtimeVersion, enabled, acceptRisk) {
            return this.change(result => this.configure(async () => {
                await setProfileVersionExemption(this.profile.dir, packageVersion, runtimeVersion, enabled, acceptRisk === true);
                Object.assign(result, await this.reload());
            }), { stage: 'enable', target: packageVersion, enabled }, 'bundle', exemptionOperationIdentity(packageVersion, runtimeVersion));
        }
        /** Read current plugins, including why a row cannot be changed through the profile patch.
         * @returns Current runtime entries with persistent patch targets.
         */
        async listPlugins() {
            const rows = flatten(composeEntries([readProfilePatches('dsh', this.profile)]));
            const snapshot = await readPluginInventory(this.ctx);
            return snapshot.entries.map((entry) => {
                const actual = [...this.ctx.loader.entries()].find(row => row.id === entry.entryId);
                const candidates = rows.filter(row => row.id === actual?.options.id);
                const candidate = candidates[0];
                if (PROTECTED_PLUGINS.has(entry.moduleName) || entry.entryId === this.ownerEntryId) {
                    return { ...entry, readOnlyReason: 'management-required' };
                }
                if (candidate === undefined || candidates.length > 1 || candidate.name !== entry.moduleName
                    || actual?.parent.tree.ctx.fiber.entry?.id !== 'include') {
                    return { ...entry, readOnlyReason: 'unaddressable' };
                }
                return { ...entry, patchId: candidate.id };
            });
        }
        /** Read installed, installation-provided, and offline Official catalog bundles, plus selected non-bundle names.
         * A dependency without a bundle patch is listed, as a `not-bundle` problem, only while it is selected.
         * @returns Package versions, manifest descriptions, the installable spec of profile dependencies, rows, optional
         * display metadata, activation selections, whether the installation offers the bundle, and removal availability.
         */
        listBundles() {
            return Promise.resolve(this.readBundles().map(bundle => ({ ...bundle, updatable: typeof updateRefusal(bundle, bundle.name) !== 'string' })));
        }
        /** The bundle inventory before update eligibility, which is derived from each listed bundle. */
        readBundles() {
            const manifest = readProfileManifest('dsh', this.profile.dir);
            const exemptions = readProfileVersionExemptions(this.profile.dir);
            const selected = bundleSelection(manifest);
            const recorded = manifest.dependencies ?? {};
            const dependencies = Object.keys(recorded);
            const installation = JSON.parse(readFileSync(this.profile.installAnchor, 'utf8'));
            const catalog = new Map(OFFICIAL_ON_DEMAND_CATALOG.map(entry => [entry.packageName, entry]));
            const names = [...new Set([...selected, ...dependencies, ...Object.keys(installation.dependencies ?? {}), ...catalog.keys()])];
            const bundles = [];
            const runtimeVersion = getDshRuntimeVersion();
            for (const name of names) {
                const installed = dependencies.includes(name);
                const optional = OPTIONAL_BUNDLES.includes(name);
                const enabled = selected.includes(name);
                const shipped = Object.hasOwn(installation.dependencies ?? {}, name);
                const offered = catalog.get(name);
                const official = optional || offered !== undefined;
                let target = {};
                let targetError;
                if (offered !== undefined) {
                    try {
                        target = { installTarget: officialBundleInstallTarget(name, this.profile.installAnchor, runtimeVersion) };
                    }
                    catch (error) {
                        targetError = managementError(error);
                    }
                }
                const catalogMeta = offered === undefined ? {} : { meta: offered.meta };
                if (offered !== undefined && !installed && !shipped) {
                    let platformError;
                    try {
                        assertBundlePlatform(offered.platforms, process.platform);
                    }
                    catch (error) {
                        platformError = managementError(error);
                    }
                    const error = platformError ?? targetError ?? (enabled ? { code: 'not-bundle' } : undefined);
                    bundles.push({ name, official, availability: 'missing', ...catalogMeta, ...target, enabled, installed, optional,
                        ...offered.listedByDefault === undefined ? {} : { listedByDefault: offered.listedByDefault },
                        ...platformError?.code === 'unsupported-platform' ? { listedByDefault: false } : {},
                        removable: enabled, rows: [], overrides: [],
                        ...error === undefined ? {} : { error } });
                    continue;
                }
                // Installation-owned copies remain protected even if a profile also declares their names.
                const owned = installed && !shipped;
                const removable = owned || (enabled && !installed && !shipped);
                const sourceOf = (packageName) => owned ? { source: dependencySpec(name, recorded[name], this.profile.dir, packageName) } : {};
                const readOnlyReason = this.protectsManager(name) ? 'management-required' : undefined;
                let availability = 'missing';
                let version;
                let listing = {};
                try {
                    const info = bundleManifest(name, this.profile.dir, this.profile.installAnchor);
                    availability = shipped ? 'installation' : 'profile';
                    if (info === undefined) {
                        if (enabled || offered !== undefined)
                            bundles.push({ name, official, availability, ...catalogMeta, ...target,
                                ...sourceOf(), enabled, installed, optional, removable: removable && readOnlyReason === undefined,
                                ...(readOnlyReason === undefined ? {} : { readOnlyReason }), error: targetError ?? { code: 'not-bundle' }, rows: [], overrides: [] });
                        continue;
                    }
                    version = info.version;
                    const listedByDefault = info.dsh?.bundle?.listedByDefault;
                    if (listedByDefault !== undefined) {
                        if (typeof listedByDefault !== 'boolean')
                            throw new Error('dsh.bundle.listedByDefault must be a boolean');
                        listing = { listedByDefault };
                    }
                    const compatibility = evaluatePluginCompatibility(info, exemptions);
                    if (compatibility !== undefined && !compatibility.exempted)
                        throw new ManagementFailure('incompatible-version', [incompatiblePlugin(compatibility)]);
                    const dir = resolveBundleDir('dsh', name, this.profile.installAnchor, this.profile.dir);
                    const meta = offered?.meta ?? readPluginMeta(info.name ?? name, pathToFileURL(join(dir, 'package.json')).href);
                    let platformError;
                    try {
                        assertBundlePlatform(info.dsh?.bundle?.platforms, process.platform);
                    }
                    catch (error) {
                        platformError = managementError(error);
                    }
                    if (platformError?.code === 'unsupported-platform')
                        listing = { listedByDefault: false };
                    const error = platformError ?? targetError;
                    bundles.push({ name, official, availability, ...listing, ...target, ...(version === undefined ? {} : { version }),
                        ...(info.description === undefined || info.description === '' ? {} : { description: info.description }),
                        ...meta === undefined ? {} : { meta },
                        ...sourceOf(info.name), enabled, installed, optional, removable: removable && readOnlyReason === undefined,
                        ...(readOnlyReason === undefined ? {} : { readOnlyReason }),
                        ...this.declaredRows(name, info),
                        ...error === undefined ? {} : { error } });
                }
                catch (error) {
                    if (enabled || installed || optional || offered !== undefined) {
                        bundles.push({ name, official, availability, ...listing, ...catalogMeta, ...target, ...(version === undefined ? {} : { version }),
                            ...sourceOf(), enabled, installed, optional, removable: removable && readOnlyReason === undefined,
                            ...(readOnlyReason === undefined ? {} : { readOnlyReason }),
                            error: targetError ?? managementError(error), rows: [], overrides: [] });
                    }
                }
            }
            return bundles;
        }
        /** Read the registries this manager asks: the configured first one, its fallbacks in order, and what pnpm's own configuration names.
         * @returns The registries in pnpm's comparison form; null is the one pnpm's own configuration names, `resolved` as pnpm reads it now.
         */
        async registries() {
            return {
                ...this.configuredRegistries, fallbackRegistries: [...this.configuredRegistries.fallbackRegistries],
                resolved: await readProfileRegistry(this.profile.dir, {
                    ...this.profile.packageManager ?? { command: this.pnpmCommand }, timeoutMs: this.inspectTimeoutMs,
                }),
            };
        }
        /** Read what a spec names before installing it.
         * @param spec One package spec: a registry name, an absolute path, a git address, or a tarball.
         * @param options The registry asked first.
         * @param signal Ends a registry lookup early.
         * @returns The package the spec names, or why it is refused.
         */
        async inspect(spec, options, signal) {
            let parsed;
            try {
                parsed = parseInstallSpec(spec);
            }
            catch (error) {
                /* v8 ignore next 2 -- parseInstallSpec throws nothing but its own refusal */
                if (!(error instanceof InvalidInstallSpecError))
                    throw error;
                return refused('invalid-spec', error.reason);
            }
            const manifest = readProfileManifest('dsh', this.profile.dir);
            const installation = JSON.parse(readFileSync(this.profile.installAnchor, 'utf8'));
            const known = new Set([
                ...bundleSelection(manifest), ...Object.keys(manifest.dependencies ?? {}), ...Object.keys(installation.dependencies ?? {}),
            ]);
            const plan = registryPlan(options?.registry, await this.registries());
            const registry = plan[0];
            switch (parsed.kind) {
                case 'git': return { status: 'accepted', kind: 'git', bundle: null, registry, host: parsed.host };
                case 'tarball':
                    if (parsed.path !== undefined && !existsSync(parsed.path))
                        return refused('not-a-package', 'the tarball does not exist');
                    return { status: 'accepted', kind: 'tarball', bundle: null, registry, ...parsed.host === undefined ? {} : { host: parsed.host } };
                case 'path': {
                    if (!existsSync(parsed.path))
                        return refused('not-a-package', 'the path does not exist');
                    let read;
                    try {
                        read = JSON.parse(await readFile(join(parsed.path, 'package.json'), 'utf8'));
                    }
                    catch (error) {
                        return refused('not-a-package', `no readable package.json at the path: ${messageOf(error)}`);
                    }
                    const inspection = inspectionOf('path', read, registry);
                    if (inspection.name === undefined)
                        return refused('not-a-package', 'the package.json names no package');
                    if (known.has(inspection.name))
                        return refused('already-installed', `${inspection.name} is already installed`);
                    if (!inspection.bundle)
                        return refused('not-a-bundle', `${inspection.name} declares no dsh.bundle`);
                    return inspection;
                }
                case 'registry': {
                    if (known.has(parsed.name))
                        return refused('already-installed', `${parsed.name} is already installed`);
                    const asked = await this.askRegistries(spec.trim(), INSPECT_FIELDS, plan, parsed, signal);
                    const registries = asked.attempts.map(attempt => attempt.registry);
                    if (asked.status === 'refused')
                        return { status: 'refused', problem: asked.problem, reason: asked.reason, registries };
                    // pnpm prints a range's highest match as one object; npm's array of every match is read by its last entry.
                    const latest = Array.isArray(asked.answer) ? asked.answer.at(-1) : asked.answer;
                    if (typeof latest !== 'object' || latest === null)
                        return { status: 'refused', problem: 'unknown', reason: 'pnpm view answered no package', registries };
                    const inspection = inspectionOf('registry', latest, asked.registry);
                    const named = inspection.name === undefined ? { ...inspection, name: parsed.name } : inspection;
                    if (!named.bundle)
                        return { status: 'refused', problem: 'not-a-bundle', reason: `${named.name} declares no dsh.bundle`, registries };
                    return named;
                }
            }
        }
        /** Read the published versions newer than an installed registry package's version, from the first registry that answers.
         * Official bundles follow the DSH version and are not listed.
         * @param name Installed profile dependency name.
         * @param options The registry asked first; absent, the one the package was installed from in this process, else the configured one.
         * @param signal Ends the lookups early.
         * @returns At most `versionListLimit` newer versions, newest first, with their tags, deprecation, bundle declaration,
         * and DSH compatibility, and the registry that answered; or why no list could be read, with every registry asked.
         */
        async listVersions(name, options, signal) {
            const target = updateRefusal((await this.listBundles()).find(bundle => bundle.name === name), name);
            if (typeof target === 'string')
                return { status: 'refused', problem: 'not-updatable', reason: target, attempts: [] };
            const plan = registryPlan(this.registryFor(name, options?.registry), await this.registries());
            const listing = await this.askRegistries(name, VERSION_LIST_FIELDS, plan, target.parsed, signal);
            if (listing.status === 'refused')
                return listing;
            let newer;
            try {
                newer = newerVersions(listing.answer, target.installed);
            }
            catch (error) {
                return { status: 'refused', problem: 'unknown', reason: messageOf(error), attempts: listing.attempts };
            }
            const listed = newer.slice(0, this.versionListLimit);
            const exemptions = readProfileVersionExemptions(this.profile.dir);
            const looked = await mapConcurrent(listed, this.versionLookupConcurrency, async (published) => {
                const spec = `${name}@${published.version}`;
                const answer = await this.askRegistries(spec, VERSION_FIELDS, [listing.registry], { kind: 'registry', spec, name }, signal);
                if (answer.status === 'refused')
                    return answer;
                const facts = versionFacts(answer.answer, name, published.version);
                let compatibility;
                try {
                    compatibility = evaluatePluginCompatibility(facts.manifest, exemptions);
                }
                catch (error) {
                    return { status: 'refused', problem: 'unknown', reason: messageOf(error), attempts: answer.attempts };
                }
                const version = {
                    ...published, bundle: facts.bundle, ...facts.deprecated === undefined ? {} : { deprecated: facts.deprecated },
                    ...compatibility === undefined || compatibility.exempted ? {} : { incompatible: incompatiblePlugin(compatibility) },
                };
                return { status: 'read', version };
            }, answer => answer.status === 'refused');
            const versions = [];
            for (const answer of looked) {
                /* v8 ignore next -- a lookup stops the rest only by refusing, which returns below */
                if (answer === undefined)
                    continue;
                if (answer.status === 'refused')
                    return { ...answer, attempts: [...listing.attempts.slice(0, -1), ...answer.attempts] };
                versions.push(answer.version);
            }
            return {
                status: 'listed', installed: target.installed, registry: listing.registry, attempts: listing.attempts,
                versions, omitted: newer.length - listed.length,
            };
        }
        /**
         * Replace an installed registry bundle with a newer published version. The target is checked on the registry first:
         * it must exist, declare a bundle, and accept the running DSH version, or nothing changes. With HMR the installed
         * version is removed, so its plugins stop, and the target is installed with the same selection; when that
         * installation fails or is cancelled, the removed version is installed again from the registry it came from.
         * Without HMR the target is installed over the running version and the result reports the restart.
         * @param name Installed profile dependency name.
         * @param version Exact published version newer than the installed one.
         * @param options The registry asked first, as for `listVersions`, and the request id naming the progress and the
         * target installation's logs and cancellation.
         * @returns The versions, the last step that ran with its result, and the reinstallation when one ran.
         */
        async updateBundle(name, version, options) {
            const requestId = options?.requestId;
            const announce = (step) => {
                if (requestId !== undefined)
                    this.ownerContext.emit('plugin-manager/update-state', { requestId, step });
            };
            announce('checking');
            const target = updateRefusal((await this.listBundles()).find(item => item.name === name), name);
            const from = typeof target === 'string' ? '' : target.installed;
            const refusedBefore = (error) => ({
                from, to: version, step: 'check',
                result: { changed: false, application: 'failed', stage: 'install', target: name, error },
            });
            if (typeof target === 'string')
                return refusedBefore({ code: 'invalid-spec', diagnostic: target });
            const { bundle } = target;
            if (semver.valid(version) === null || !semver.gt(version, from)) {
                return refusedBefore({ code: 'invalid-spec', diagnostic: `${version} is not a version newer than the installed ${from}` });
            }
            const spec = `${name}@${version}`;
            const plan = registryPlan(this.registryFor(name, options?.registry), await this.registries());
            const checked = await this.askRegistries(spec, VERSION_FIELDS, plan, { kind: 'registry', spec, name });
            if (checked.status === 'refused')
                return refusedBefore({ code: 'operation-error', diagnostic: checked.reason });
            const facts = versionFacts(checked.answer, name, version);
            if (!facts.bundle)
                return refusedBefore({ code: 'not-bundle' });
            let compatibility;
            try {
                compatibility = evaluatePluginCompatibility(facts.manifest, readProfileVersionExemptions(this.profile.dir));
            }
            catch (error) {
                return refusedBefore({ code: 'operation-error', diagnostic: messageOf(error) });
            }
            if (compatibility !== undefined && !compatibility.exempted) {
                return refusedBefore({ code: 'incompatible-version', incompatible: [incompatiblePlugin(compatibility)] });
            }
            const { enabled } = bundle;
            // A lock timeout or disposal rejects instead of reporting; it reads as the failed step it is.
            const settled = (stage, target, operation) => operation.catch((error) => ({ changed: false, application: 'failed', stage, target, error: managementError(error) }));
            const installTarget = () => settled('install', spec, this.installBundle(spec, {
                saveExact: true, registry: checked.registry, enabled, ...requestId === undefined ? {} : { requestId },
            }));
            if (this.ownerContext.get('hmr') === undefined) {
                announce('installing');
                return { from, to: version, step: 'install', result: await installTarget() };
            }
            // The removed version is installed again where it came from, else over the configured plan, as its removal asks.
            const previousRegistry = this.installedFrom.get(name);
            announce('removing');
            const removal = await settled('remove', name, this.removeBundle(name));
            if (removal.application === 'failed' || removal.application === 'cancelled')
                return { from, to: version, step: 'remove', result: removal };
            announce('installing');
            const result = await installTarget();
            if (result.application !== 'failed' && result.application !== 'cancelled')
                return { from, to: version, step: 'install', result };
            announce('restoring');
            // A target that installed but failed to start is still a dependency; it goes before the removed version returns.
            if (result.stage === 'enable')
                await settled('remove', name, this.removeBundle(name));
            // The reinstallation carries no request id, so cancelling the update cannot leave the package removed.
            const previous = `${name}@${from}`;
            const restore = await settled('install', previous, this.installBundle(previous, {
                saveExact: true, enabled, ...previousRegistry === undefined ? {} : { registry: previousRegistry },
            }));
            return { from, to: version, step: 'install', result, restore };
        }
        /**
         * The registry a version listing or an update asks first.
         * @param name The package name.
         * @param requested The caller's registry, when it named one.
         * @returns The requested registry, else the one this process last installed the package from, else undefined for the configured one.
         */
        registryFor(name, requested) {
            return requested !== undefined ? requested : this.installedFrom.get(name);
        }
        /**
         * Ask registries in turn through `pnpm view`, moving to the next one while a failure is the registry's: unreachable,
         * timed out, or without the package or version, which a mirror not yet synced reports.
         * @param spec The registry spec viewed.
         * @param fields The manifest fields printed.
         * @param plan The registries, in order; never empty.
         * @param parsed The spec's form, which decides whether a failure was the registry's.
         * @param signal Ends the lookup; a dropped lookup is not carried to the next registry.
         * @returns The parsed answer and the registry that gave it, or the last refusal, with every registry asked.
         */
        async askRegistries(spec, fields, plan, parsed, signal) {
            const attempts = [];
            for (const [index, registry] of plan.entries()) {
                const view = await viewProfilePackage(this.profile.dir, spec, {
                    ...this.profile.packageManager ?? { command: this.pnpmCommand },
                    fields, timeoutMs: this.inspectTimeoutMs, ...signal === undefined ? {} : { signal }, registry,
                });
                const printed = view.stdout.replace(ANSI_SEQUENCE, '').trim();
                if (view.exitCode !== 0 || view.cause !== undefined || view.timedOut) {
                    // pnpm prints a refusal as `{ error: { code, message } }` on stdout, with nothing on stderr.
                    const log = [view.stderr.trim(), printedError(printed), view.cause === undefined ? '' : messageOf(view.cause)].filter(Boolean).join('\n');
                    const kind = classifyInstallFailure({ log, timedOut: view.timedOut, ...view.cause === undefined ? {} : { cause: view.cause } });
                    const reason = view.timedOut ? `pnpm view timed out after ${String(this.inspectTimeoutMs)}ms` : log || printed || `pnpm view exited with ${String(view.exitCode)}`;
                    const problem = kind === 'not-found' || kind === 'no-matching-version' ? 'not-found' : kind === 'network' || kind === 'timeout' ? 'network' : 'unknown';
                    attempts.push({ registry, problem, reason });
                    if (index < plan.length - 1 && signal?.aborted !== true && attributeFailure(kind, log, parsed) === 'registry')
                        continue;
                    return { status: 'refused', problem, reason, attempts };
                }
                try {
                    const answer = JSON.parse(printed || 'null');
                    attempts.push({ registry });
                    return { status: 'answered', answer, registry, attempts };
                }
                catch (error) {
                    const reason = `unreadable pnpm view output: ${messageOf(error)}`;
                    attempts.push({ registry, problem: 'unknown', reason });
                    return { status: 'refused', problem: 'unknown', reason, attempts };
                }
            }
            /* v8 ignore next -- the plan is never empty: every attempt returns or continues to the next */
            throw new Error('no registry was asked');
        }
        /** Persist a plugin entry's desired enablement and apply it on live profiles.
         * @param id Loader entry identity returned by listPlugins.
         * @param enabled Whether the plugin should run.
         * @returns Saved and runtime outcomes, including higher-priority overrides.
         */
        setPluginEnabled(id, enabled) {
            return this.change(result => this.configure(async () => {
                const row = (await this.listPlugins()).find(item => item.entryId === id);
                if (row === undefined)
                    throw new ManagementFailure('unknown-plugin');
                if (row.readOnlyReason !== undefined)
                    throw new ManagementFailure(row.readOnlyReason);
                await writePluginEnabled(this.profile, row.patchId, row.moduleName, enabled);
                Object.assign(result, await this.reload(enabled ? [row.patchId] : []));
                const current = (await this.listPlugins()).find(item => item.entryId === id);
                return current?.enabled !== enabled && this.ownerContext.get('hmr') !== undefined ? 'overridden' : undefined;
            }), { stage: 'enable', target: id, enabled }, 'plugin');
        }
        /** Select or remove a bundle layer while retaining installed dependencies.
         * @param name Bundle package name.
         * @param enabled Whether the bundle contributes its patch layer.
         * @returns Persisted and runtime outcomes.
         */
        setBundleEnabled(name, enabled) {
            return this.change(result => this.configure(async () => {
                await this.selectBundle(name, enabled);
                Object.assign(result, enabled ? await this.publishAndReload(this.bundleRows(name).map(row => row.id)) : await this.reload());
            }), { stage: 'enable', target: name, enabled }, 'bundle');
        }
        /**
         * Install a package using the same pnpm implementation as dsh plugin. GitHub
         * repositories get a connection check bounded by githubConnectionTimeoutMs before pnpm starts;
         * only network failures or timeouts stop installation, and a stop streams the check's diagnostic
         * as the installation's install-log run under its request id, while pnpm owns authentication and transport fallback. An
         * HTTP(S) Git URL on another host is installed over SSH instead when access probes bounded by gitTransportTimeoutMs
         * find that only SSH can read it. A run
         * that fails, is cancelled, or adds a package without a bundle patch restores
         * `package.json` and `pnpm-lock.yaml` as they were; downloaded files can stay.
         * @param spec One package spec, including local paths relative to the invocation directory.
         * @param options Whether to activate the installed bundle (defaults to true), the request id a cancellation names,
         * the pending build scripts to allow, whether to save an exact dependency, and the registry asked first.
         * @returns Package-manager diagnostics, the registries asked, and the observed activation outcome.
         */
        installBundle(spec, options) {
            const requestId = options?.requestId;
            const control = { abort: new AbortController(), phase: 'installing', result: Promise.resolve(null) };
            const stopped = () => control.abort.signal.aborted || this.abort.signal.aborted;
            if (requestId !== undefined)
                this.installs.set(requestId, control);
            const announce = (phase, attempt) => {
                if (requestId !== undefined)
                    this.ownerContext.emit('plugin-manager/install-state', { requestId, phase, ...attempt === undefined ? {} : { attempt } });
            };
            const result = this.change(async (result) => {
                if (spec.trim() === '' || spec.startsWith('-'))
                    throw new ManagementFailure('invalid-spec');
                if (stopped())
                    throw new InstallCancelledError();
                const before = readProfileManifest('dsh', this.profile.dir).dependencies ?? {};
                // With HMR, an installed package is removed before another version is installed, so its plugins stop first.
                const known = specPackageName(spec, before, this.profile.dir);
                if (this.ownerContext.get('hmr') !== undefined && known !== undefined && Object.hasOwn(before, known)) {
                    throw new ManagementFailure('already-installed');
                }
                if (options?.approvedBuilds !== undefined) {
                    await approveBuilds(this.profile.dir, options.approvedBuilds);
                    result.approvedBuilds = options.approvedBuilds;
                }
                const files = await this.readRestoredFiles();
                let name;
                let version;
                try {
                    result.registries = [];
                    const parsed = parsedForRegistry(spec);
                    const signal = AbortSignal.any([this.abort.signal, control.abort.signal]);
                    const env = this.profile.packageManager?.env === undefined ? {} : { env: this.profile.packageManager.env };
                    const connection = checkGithubConnection(parsed, this.profile.dir, {
                        timeoutMs: this.githubConnectionTimeoutMs, outputBytes: this.outputBytes, signal, ...env,
                    });
                    this.packageOperations.add(connection);
                    let connectionFailure;
                    try {
                        connectionFailure = await connection;
                    }
                    finally {
                        this.packageOperations.delete(connection);
                    }
                    if (stopped())
                        throw new InstallCancelledError();
                    if (connectionFailure?.kind === 'network' || connectionFailure?.kind === 'timeout') {
                        // The answer keeps a plain PackageResult; the check's command line is only streamed as the run below.
                        const { argv, ...packageResult } = connectionFailure;
                        result.packageResult = packageResult;
                        result.failedAt = 'spec-host';
                        // pnpm never runs, so the check is the installation's only run: stream its failure into the install log.
                        // A timed-out Git reports no exit code of its own, unlike the result, which records the status it is read as.
                        if (requestId !== undefined) {
                            this.ownerContext.emit('plugin-manager/install-log', {
                                requestId, jobId: randomUUID(), argv, cwd: this.profile.dir, stream: 'stderr',
                                text: connectionFailure.output, exitCode: connectionFailure.timedOut === true ? null : connectionFailure.exitCode,
                            });
                        }
                        throw new Error(connectionFailure.output);
                    }
                    const transport = chooseGitTransport(parsed, this.profile.dir, { timeoutMs: this.gitTransportTimeoutMs, signal, ...env });
                    this.packageOperations.add(transport);
                    let installSpec;
                    try {
                        installSpec = await transport;
                    }
                    finally {
                        this.packageOperations.delete(transport);
                    }
                    if (stopped())
                        throw new InstallCancelledError();
                    // The last run is the result's; the registries asked stay listed whatever the outcome.
                    const plan = registryPlan(options?.registry, await this.registries());
                    let run;
                    let used = null;
                    for (const [index, registry] of plan.entries()) {
                        if (index > 0)
                            await this.restoreFiles(files);
                        // A stop that landed while the files went back, or before the first run, starts no run with a dead signal.
                        if (stopped())
                            throw new InstallCancelledError();
                        result.registries.push(registry);
                        used = registry;
                        announce('installing', { registry, index: index + 1, total: plan.length });
                        run = await this.runPnpm(['add', installSpec, ...options?.saveExact === true ? ['--save-exact'] : [], ...registryArguments(registry)], control.abort.signal, requestId);
                        result.packageResult = run;
                        if (stopped())
                            throw new InstallCancelledError();
                        // A compatibility refusal is the package's own answer, so no other registry is asked.
                        if (run.incompatible !== undefined)
                            throw new ManagementFailure('incompatible-version', run.incompatible);
                        // A run this manager terminated is not a success, even when pnpm trapped the signal and exited 0.
                        if (run.exitCode === 0 && run.timedOut !== true)
                            break;
                        /* v8 ignore next 2 -- runPnpm classifies every run it does not report as succeeded */
                        if (run.kind === undefined)
                            break;
                        // What the last failed run could not reach; a later run that succeeds leaves nothing to say.
                        delete result.failedAt;
                        // A run this manager terminated got no answer from the registry at all, so no registry explains it.
                        if (run.timedOut === true)
                            break;
                        const failedAt = attributeFailure(run.kind, run.output, parsedForRegistry(installSpec));
                        if (failedAt !== 'other')
                            result.failedAt = failedAt;
                        if (failedAt !== 'registry' || index === plan.length - 1)
                            break;
                    }
                    /* v8 ignore next -- the plan is never empty, so a run always settled */
                    if (run === undefined)
                        throw new Error('no registry was asked');
                    // A terminated run reports no usable exit status, so neither its files nor its bundle are trusted.
                    const succeeded = run.exitCode === 0 && run.timedOut !== true;
                    if (succeeded)
                        delete result.failedAt;
                    if (!succeeded) {
                        // pnpm-workspace.yaml is not restored, so the names it left undecided, or reported as blocked, are offered for approval.
                        try {
                            result.pendingBuilds = await recordPendingBuilds(this.profile.dir, run.output);
                        }
                        catch (error) {
                            this.ownerContext.logger.warn('Could not read pending build approvals after pnpm failed', error);
                        }
                        throw new Error(run.output);
                    }
                    const after = readProfileManifest('dsh', this.profile.dir).dependencies ?? {};
                    const installed = Object.keys(after).filter(name => before[name] !== after[name]);
                    // Without HMR a repeated installation can leave the saved dependency as it was.
                    if (installed.length === 0) {
                        const local = specPackageName(spec, after, this.profile.dir);
                        if (local !== undefined && Object.hasOwn(after, local))
                            installed.push(local);
                    }
                    const target = installed[0];
                    if (installed.length !== 1 || target === undefined)
                        throw new ManagementFailure('ambiguous-install');
                    name = target;
                    const dir = resolveBundleDir('dsh', name, this.profile.installAnchor, this.profile.dir);
                    const manifest = bundleManifest(name, this.profile.dir, this.profile.installAnchor);
                    if (manifest?.dsh?.bundle === undefined)
                        throw new ManagementFailure('not-bundle');
                    const compatibility = evaluatePluginCompatibility(manifest, readProfileVersionExemptions(this.profile.dir));
                    if (compatibility !== undefined && !compatibility.exempted)
                        throw new ManagementFailure('incompatible-version', [incompatiblePlugin(compatibility)]);
                    for (const file of bundlePatchPaths(dir, manifest.dsh.bundle))
                        loadOverlayPatches('dsh', file);
                    version = manifest.version;
                    this.installedFrom.set(name, used);
                }
                catch (error) {
                    // pnpm has exited by now, so the files it rewrote go back as they were.
                    await this.restoreFiles(files);
                    throw error;
                }
                control.phase = 'applying';
                announce('applying');
                result.bundle = name;
                if (version !== undefined)
                    result.version = version;
                result.target = name;
                result.stage = 'enable';
                return this.configure(async () => {
                    if (options?.enabled !== false)
                        await this.selectBundle(name, true);
                    // Without HMR the replaced package's plugins keep running until the restart the result already reports.
                    if (Object.hasOwn(before, name) && this.ownerContext.get('hmr') === undefined)
                        return;
                    // A selected bundle's own rows must start.
                    const selected = bundleSelection(readProfileManifest('dsh', this.profile.dir)).includes(name);
                    Object.assign(result, await this.publishAndReload(selected ? this.bundleRows(name).map(row => row.id) : []));
                });
            }, { stage: 'install', target: spec, enabled: options?.enabled !== false }, 'install');
            control.result = result;
            return result.finally(() => { if (requestId !== undefined)
                this.installs.delete(requestId); });
        }
        /** Recover the result of an active installation without cancelling it.
         * @param requestId The id supplied when installation started.
         * @returns The installation's outcome after it settles, or null if no active request has that id.
         * Completed results are not retained; null establishes neither success nor cancellation.
         */
        async waitForInstall(requestId) {
            return this.installs.get(requestId)?.result ?? null;
        }
        /** Stop an installation this manager owns and wait until its files are back.
         * @param requestId The id the installation was started with.
         * @returns `cancelled` once the Git check or pnpm exited and the files are restored, `too-late` once the bundle is being
         * applied, `not-running` for any other id.
         */
        async cancelInstall(requestId) {
            const control = this.installs.get(requestId);
            if (control === undefined)
                return { status: 'not-running' };
            if (control.phase === 'applying')
                return { status: 'too-late' };
            this.ownerContext.emit('plugin-manager/install-state', { requestId, phase: 'cancelling' });
            control.abort.abort();
            /* v8 ignore next -- change() folds every failure into its result; only a lock or disposal error rejects */
            await control.result.then(() => undefined, () => undefined);
            return { status: 'cancelled' };
        }
        /** Unload and remove a profile-owned bundle dependency through dsh plugin's pnpm path; a selected name no
         * dependency holds is only deselected.
         * @param name Installed dependency or selected bundle name.
         * @returns Removal diagnostics and the remaining profile state.
         */
        removeBundle(name) {
            return this.change(async (result) => {
                const installed = await this.configure(async () => {
                    const bundle = (await this.listBundles()).find(item => item.name === name);
                    if (bundle === undefined || !bundle.removable)
                        throw new ManagementFailure('not-removable');
                    if (this.ownerContext.get('hmr') === undefined && (this.profile.startedBundles.includes(name)
                        || (bundle.error === undefined && this.bundleRows(name).some(row => [...this.ctx.loader.entries()]
                            .some(entry => entry.options.id === row.id && entry.fiber !== undefined))))) {
                        throw new ManagementFailure('stop-profile');
                    }
                    const contributions = bundle.error === undefined ? this.bundleRows(name) : [];
                    if (bundle.enabled) {
                        await this.selectBundle(name, false);
                        Object.assign(result, await this.reload());
                    }
                    if ([...this.ctx.loader.entries()].some(entry => entry.fiber?.uid != null
                        && contributions.some(row => row.id === entry.options.id && row.name === entry.options.name))) {
                        throw new ManagementFailure('bundle-in-use');
                    }
                    return bundle.installed;
                });
                if (!installed)
                    return;
                // `pnpm remove` takes no `--registry` option; the generic configuration flag sets the same setting. A package
                // installed before this process started is removed with the registry an installation asks first.
                const registry = this.installedFrom.has(name) ? this.installedFrom.get(name) : this.configuredRegistries.registry;
                result.packageResult = await this.runPnpm(['remove', name, ...typeof registry === 'string' ? [`--config.registry=${registry}`] : []]);
                if (result.packageResult.exitCode !== 0 || result.packageResult.timedOut === true) {
                    throw new Error(result.packageResult.output);
                }
                this.installedFrom.delete(name);
                Object.assign(result, await this.configure(() => this.publishAndReload()));
            }, { stage: 'remove', target: name }, 'remove');
        }
        /** The rows a bundle's patch inserts and the existing rows it changes; an unreadable patch throws. */
        declaredRows(name, info) {
            const bundle = info.dsh?.bundle;
            /* v8 ignore next -- bundleManifest answers only manifests that declare a patch */
            if (bundle === undefined)
                return { rows: [], overrides: [] };
            const dir = resolveBundleDir('dsh', name, this.profile.installAnchor, this.profile.dir);
            const patches = bundlePatchPaths(dir, bundle).flatMap(file => loadOverlayPatches('dsh', file));
            // One entry per row id: the Loader keeps a single entry for an id, whichever layer declared it last.
            const live = new Map();
            for (const entry of this.ctx.loader.entries()) {
                /* v8 ignore next -- the Loader gives every entry an id before it is listed */
                if (typeof entry.options.id === 'string')
                    live.set(entry.options.id, {
                        entryId: pluginEntryId(entry.id), baseUrl: entry.parent.tree.ctx.baseUrl,
                    });
            }
            const rows = [];
            const packages = this.ctx.get('pluginPackages');
            for (const row of flatten(composeEntries([patches.filter(item => item.insert !== undefined)]))) {
                // Group rows are containers without a plugin inventory entry; their children carry the switches.
                if (typeof row.id !== 'string' || typeof row.name !== 'string' || row.group === true)
                    continue;
                const active = live.get(row.id);
                const entryId = active?.entryId;
                const base = active?.baseUrl ?? pathToFileURL(join(dir, 'package.json')).href;
                const meta = packages?.metaOf(row.name, base);
                rows.push({ rowId: row.id, moduleName: row.name,
                    ...entryId === undefined ? {} : { entryId }, ...meta === undefined ? {} : { meta } });
            }
            const declared = new Set(rows.map(row => row.rowId));
            const overrides = [...new Set(patches.flatMap(item => item.insert === undefined && typeof item.id === 'string' && !declared.has(item.id) ? [item.id] : []))];
            return { rows, overrides };
        }
        /** Run one pnpm command in the profile, streaming its output as install-log chunks. */
        async runPnpm(args, signal, requestId) {
            const jobId = randomUUID();
            const argv = ['pnpm', ...args];
            const cwd = this.profile.dir;
            const identity = requestId === undefined ? {} : { requestId };
            const task = runProfilePnpm({ ...this.profile, profile: this.profile.name }, args, {
                execution: 'service', ...this.profile.packageManager ?? { command: this.pnpmCommand },
                signal: signal === undefined ? this.abort.signal : AbortSignal.any([this.abort.signal, signal]),
                outputBytes: this.outputBytes, activateNewBundles: false, idleTimeoutMs: this.idleTimeoutMs,
                lookupTimeoutMs: this.inspectTimeoutMs,
                onOutput: (text, stream) => {
                    this.ownerContext.emit('plugin-manager/install-log', { ...identity, jobId, argv, cwd, stream, text });
                },
            });
            this.packageOperations.add(task);
            try {
                const result = await task;
                this.ownerContext.emit('plugin-manager/install-log', {
                    ...identity, jobId, argv, cwd, stream: 'stdout', text: '', exitCode: signal?.aborted === true ? null : result.exitCode,
                });
                // A terminated run keeps its own kind even when pnpm trapped the signal and exited 0.
                if (result.exitCode === 0 && result.timedOut !== true)
                    return result;
                return {
                    ...result,
                    kind: classifyInstallFailure({ log: result.output, ...result.timedOut === true ? { timedOut: true } : {} }),
                };
            }
            catch (error) {
                this.ownerContext.emit('plugin-manager/install-log', { ...identity, jobId, argv, cwd, stream: 'stderr', text: messageOf(error), exitCode: null });
                throw error;
            }
            finally {
                this.packageOperations.delete(task);
            }
        }
        /** The profile files an installation may rewrite, as they are now; absent files read as undefined. */
        async readRestoredFiles() {
            const files = new Map();
            for (const name of RESTORED_FILES) {
                const path = join(this.profile.dir, name);
                files.set(path, existsSync(path) ? await readFile(path, 'utf8') : undefined);
            }
            return files;
        }
        /** Put the profile files back; pnpm has exited by the time this runs. */
        async restoreFiles(files) {
            for (const [path, content] of files) {
                if (content === undefined)
                    await rm(path, { force: true });
                else
                    await writeFileAtomic(path, content, { mode: 0o600 });
            }
        }
        async selectBundle(name, enabled) {
            const manifest = readProfileManifest('dsh', this.profile.dir);
            const previous = bundleSelection(manifest);
            if (enabled || !previous.includes(name)) {
                const metadata = bundleManifest(name, this.profile.dir, this.profile.installAnchor);
                if (metadata === undefined)
                    throw new ManagementFailure('not-bundle');
                if (enabled) {
                    assertBundlePlatform(metadata.dsh?.bundle?.platforms, process.platform);
                    const compatibility = evaluatePluginCompatibility(metadata, readProfileVersionExemptions(this.profile.dir));
                    if (compatibility !== undefined && !compatibility.exempted)
                        throw new ManagementFailure('incompatible-version', [incompatiblePlugin(compatibility)]);
                    this.bundleRows(name);
                }
            }
            if (!enabled && previous.includes(name)) {
                if (this.protectsManager(name))
                    throw new ManagementFailure('management-required');
            }
            const bundles = enabled ? [...previous, ...previous.includes(name) ? [] : [name]] : previous.filter(item => item !== name);
            if (JSON.stringify(previous) === JSON.stringify(bundles))
                return;
            manifest.dsh = { ...manifest.dsh, profile: { ...manifest.dsh?.profile, bundles } };
            await saveManifest(this.profile.dir, manifest);
            if (enabled)
                this.protectsManager(name);
        }
        bundleRows(name) {
            const info = bundleManifest(name, this.profile.dir, this.profile.installAnchor);
            if (info?.dsh?.bundle === undefined)
                return [];
            const dir = resolveBundleDir('dsh', name, this.profile.installAnchor, this.profile.dir);
            return flatten(composeEntries([bundlePatchPaths(dir, info.dsh.bundle).flatMap(file => loadOverlayPatches('dsh', file))]));
        }
        protectsManager(name) {
            if (this.managementBundles.has(name))
                return true;
            let rows;
            try {
                rows = this.bundleRows(name);
            }
            catch (_error) {
                // Unreadable bundles contribute no new rows; listBundles reports their diagnostics.
                return false;
            }
            const protectedBundle = rows.some(row => PROTECTED_PLUGINS.has(row.name) || `include:${row.id}` === this.ownerEntryId);
            if (protectedBundle)
                this.managementBundles.add(name);
            return protectedBundle;
        }
        configure(operation) {
            const hmr = this.ownerContext.get('hmr');
            const apply = () => { this.abort.signal.throwIfAborted(); return operation(); };
            return hmr === undefined ? apply() : hmr.runExclusive(apply);
        }
        /**
         * Publish the profile's runtime resolution without HMR, unless a deselected startup bundle still runs and needs the
         * current table. A publication that needs a restart waits for the restart the result already reports.
         */
        async refreshPackages() {
            const selected = bundleSelection(readProfileManifest('dsh', this.profile.dir));
            if (this.profile.startedBundles.some(name => !selected.includes(name)))
                return;
            try {
                const packages = this.ownerContext.get('pluginPackages');
                // A dropped mapping's bundle is not running: the guard above keeps the table while a deselected startup bundle runs.
                if (packages?.resolution !== undefined)
                    await packages.refresh();
            }
            catch (error) {
                if (!(error instanceof ResolutionRestartRequired))
                    throw error;
            }
        }
        /**
         * Apply a package change: without HMR, publish the runtime resolution when that is safe, then {@link reload}.
         * @param requiredIds Entry ids that must start.
         * @returns The reload's outcome.
         */
        async publishAndReload(requiredIds = []) {
            if (this.ownerContext.get('hmr') === undefined)
                await this.refreshPackages();
            return this.reload(requiredIds);
        }
        /**
         * Apply the saved profile; with HMR it recomposes live, including the runtime resolution.
         * @param requiredIds Entry ids that must start.
         * @returns Warnings about entries that were already inactive, and why the change waits for a restart, if it does.
         */
        async reload(requiredIds = []) {
            const hmr = this.ownerContext.get('hmr');
            if (hmr === undefined) {
                // Startup-only profiles recompose at restart; the saved patches still have to compose.
                composeEntries([readProfilePatches('dsh', this.profile)]);
                return { warnings: [] };
            }
            const { warnings, pending, restartReason } = await hmr.reconcileProfile(requiredIds);
            const restart = restartReason ?? pending[0]?.reason;
            return restart === undefined ? { warnings } : { warnings, restartReason: restart };
        }
        async change(operation, request, reason, identity) {
            const id = randomUUID();
            const target = identity === undefined ? this.operationTarget(reason, request.target) : identity.target;
            const version = identity === undefined ? this.operationVersion(reason, reason === 'plugin' ? request.target : target) : identity.version;
            const facts = { id, reason, stage: request.stage,
                target,
                ...version === undefined ? {} : { version },
                ...request.enabled === undefined ? {} : { enabled: request.enabled } };
            this.reportOperation({ ...facts, time: new Date().toISOString(), phase: 'started' });
            let settled;
            try {
                settled = await withFileLock(join(this.profile.dir, 'package.json'), async () => {
                    this.abort.signal.throwIfAborted();
                    const before = this.diskState();
                    const result = { ...request, changed: false,
                        ...this.ownerContext.get('hmr') !== undefined ? { application: 'applied' } : { application: 'restart-required', restartReason: 'no-hmr' } };
                    try {
                        const outcome = await operation(result);
                        if (outcome !== undefined)
                            result.application = outcome;
                        // A reload that waits for a restart leaves the change saved but not applied.
                        else if (result.application === 'applied' && result.restartReason !== undefined)
                            result.application = 'restart-required';
                    }
                    catch (error) {
                        if (error instanceof InstallCancelledError) {
                            result.application = 'cancelled';
                        }
                        else {
                            result.application = 'failed';
                            result.error = managementError(error);
                        }
                    }
                    if (result.application !== 'restart-required')
                        delete result.restartReason;
                    result.changed = before !== this.diskState();
                    this.ownerContext.emit('plugin-manager/changed', { reason });
                    return result;
                }, { waitMs: this.lockWaitMs });
            }
            catch (error) {
                this.reportOperation({ ...facts, time: new Date().toISOString(), phase: 'unknown' });
                throw error;
            }
            const refusedVersion = reason === 'install'
                ? settled.error?.incompatible?.find(plugin => plugin.name === facts.target)?.version
                : undefined;
            const observedVersion = diagnosticVersion(settled.version ?? refusedVersion);
            this.reportOperation({ ...facts, time: new Date().toISOString(), phase: 'settled',
                stage: settled.stage, target: settled.bundle === undefined ? facts.target : installOperationTarget(settled.bundle),
                application: settled.application, changed: settled.changed,
                ...observedVersion === undefined ? {} : { version: observedVersion },
                ...settled.error === undefined ? {} : { errorCode: settled.error.code },
                ...settled.restartReason === undefined ? {} : { restartReason: settled.restartReason } });
            return settled;
        }
        reportOperation(operation) {
            void this.ownerContext.parallel('plugin-manager/operation', operation).catch((_error) => {
                // All subscribers run; diagnostic failures cannot change the management result.
                console.error('dsh plugin-manager: operation diagnostic delivery failed');
            });
        }
        operationTarget(reason, target) {
            if (reason !== 'plugin') {
                const name = installOperationTarget(target);
                return reason === 'install' || name === target ? name : null;
            }
            return target.length <= 256 && [...this.ctx.loader.entries()].some(entry => entry.id === target) ? target : null;
        }
        /** Read the request-time package version without preventing recovery from damaged metadata. */
        operationVersion(reason, target) {
            if (reason === 'install' || target === null)
                return undefined;
            try {
                if (reason !== 'plugin')
                    return diagnosticVersion(bundleManifest(target, this.profile.dir, this.profile.installAnchor)?.version);
                const entry = [...this.ctx.loader.entries()].find(row => row.id === target);
                const source = entry?.parent.tree.ctx.baseUrl;
                if (entry === undefined || source === undefined)
                    return undefined;
                return diagnosticVersion(this.ownerContext.get('pluginPackages')?.packageOf(entry.options.name, source)?.version);
            }
            catch (_error) {
                // Disabling or removing a broken package must not depend on diagnostic metadata being readable.
                return undefined;
            }
        }
        diskState() {
            return ['package.json', 'cordis.patch.yml', 'pnpm-workspace.yaml', PROFILE_COMPATIBILITY_FILENAME].map((file) => {
                try {
                    return readFileSync(join(this.profile.dir, file), 'utf8');
                }
                catch (error) {
                    if (error.code === 'ENOENT')
                        return '';
                    throw error;
                }
            }).join('\0');
        }
    };
})();
export { PluginManager };
export default PluginManager;
//# sourceMappingURL=index.js.map