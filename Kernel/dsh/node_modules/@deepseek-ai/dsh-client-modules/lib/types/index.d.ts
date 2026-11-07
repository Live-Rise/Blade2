/**
 * Node half of the client module system (`dsh.client` dual-face package): scans
 * the host Loader's entries for packages declaring `dsh.client`, composes the
 * `window.__DSH_BOOT__` entry graph (wire single source: {@link WebBootEntry}
 * in `./client/manifest.ts`) in module-graph order, serves one-or-more-plugin
 * combo scripts plus their source maps, package-local chunks, and the
 * content-revisioned `./assets/*` exports of active client packages,
 * contributes the registration facade, application preloads, bootstrap scripts,
 * and graph to the webserver's index injection table, and provides the
 * `clientModules` service (the HMR node half's registration/notification
 * face).
 *
 * Package reconciliation is incremental by Loader entry name.
 * Every cordis `internal/plugin` emission (fiber construction/disposal) marks
 * the fiber's entry name dirty; a microtask flush reconciles each dirty name
 * against the Loader entries indexed at the start of that flush. Synchronous
 * package resolution must not add or remove Loader entries during reconciliation.
 * The activation pass seeds the same pending names and flushes synchronously,
 * so initial activation and steady state share one implementation. Each
 * reconciliation reads the package manifest and the bundle metadata again,
 * so an entry that restarts after its package was reinstalled or moved
 * publishes them as they are now; an unchanged result publishes nothing and
 * does not read the bundle. The manifest package name identifies the browser
 * module; distinct active Loader sources for that package are a composition
 * error. Bundle rebuilds outside
 * an entry restart reach the graph through {@link ClientModuleRegistry.rebuilt}.
 * @module @deepseek-ai/dsh-client-modules
 */
import { Service } from '@deepseek-ai/cordis';
import type { Context } from '@deepseek-ai/cordis';
import type { IndexInjection } from '@deepseek-ai/dsh-host-webserver';
import type { WebBootEntry, WebBootGraph } from './client/manifest.ts';
export { stripClientSuffix } from './client/manifest.ts';
export type { BootManifest, BootModuleRow, BootPluginRow, WebBootBatch, WebBootBatchPhase, WebBootEntry, WebBootGraph, WebBootRelease, } from './client/manifest.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** The web plugin table (provided by the client-modules node half). */
        clientModules: ClientModuleRegistry;
    }
}
/** Filesystem baseline captured before a client artifact snapshot is read. */
export interface ClientArtifactBaseline {
    /** Absolute path of the client bundle. */
    readonly path: string;
    /** Bundle modification time in milliseconds. */
    readonly mtimeMs: number;
    /** Bundle status-change time in milliseconds, including writes that preserve mtime. */
    readonly ctimeMs: number;
    /** Bundle size in bytes. */
    readonly size: number;
}
/**
 * Order composed rows so every requested dynamic package precedes its
 * consumers. An `external` specifier is either the package row it names
 * (`<pkg>/client` aliases the bare package) or a static-table name that adds no
 * graph edge.
 * @param entries - composed rows in scan order.
 * @returns the same rows reordered; scan order breaks every tie.
 * @throws {Error} when a row requests itself or when the module graph has a
 * cycle; the message lists the packages on it.
 */
export declare function orderByModuleGraph(entries: readonly WebBootEntry[]): WebBootEntry[];
/**
 * The boot protocol as index injection rows. The inline registration queue
 * precedes the application-batch preload and the blocking bootstrap batch. Its
 * `create()` method materializes the modules
 * bundle, delegates construction to that bundle, and leaves the same facade
 * in live-registration mode. The graph global follows before the shell reads
 * it.
 * @param graph - the composed entry graph.
 * @returns head rows in execution order: queue script, application preloads,
 * blocking bootstrap scripts, graph global.
 */
export declare function bootInjections(graph: WebBootGraph): IndexInjection[];
/**
 * The web plugin table service: incremental `dsh.client` scan + wire composition
 * + bundle and package-asset route + index injection rows. Construction runs
 * the activation scan synchronously — a malformed declaration or missing bundle among the
 * already-loaded entries aggregates into one loud throw (FAILED fiber; the
 * boot activation audit reports it).
 */
export declare class ClientModuleRegistry extends Service {
    static inject: string[];
    /**
     * Host release identity published in every graph: this package's version, which releases share
     * with the CLI, and the source commit the client build fixes into the built Host artifact.
     */
    private readonly release;
    private readonly table;
    private readonly sources;
    private readonly rebuildListeners;
    private readonly graphListeners;
    private readonly dirty;
    private responses;
    private batchResponses;
    /** One prior graph generation covers a request racing the HMR recomposition that replaced its URL. */
    private previousBatchResponses;
    private flushQueued;
    private composed;
    /**
     * Build the service: subscribe, seed, and run the activation flush.
     * Bundle routes follow the optional Web carrier's injected lifecycle.
     * @param ctx - plugin context carrying Loader and an optional Web carrier.
     */
    constructor(ctx: Context);
    /**
     * Current composed entry graph (stable object between changes).
     * @returns the graph served as `window.__DSH_BOOT__`.
     */
    graph(): WebBootGraph;
    /**
     * Absolute path of an entry's client bundle.
     * @param id - entry id (package name).
     * @returns the path, or undefined for an unknown id.
     */
    clientPath(id: string): string | undefined;
    /**
     * Serve an advertised revisioned bundle, source map, chunk, or package asset
     * without a Web server. Unknown URLs return 404, unsupported methods return
     * 405, and `HEAD` returns the same immutable headers without materializing a
     * body. Each bundle body is built once on its first `GET`; script
     * construction never reads maps. A package asset is read on every request
     * and answers 304 without a body when `If-None-Match` equals its entity tag;
     * its body holds the file open until the caller consumes or cancels it. A
     * bundle, map, or chunk body is the held buffer itself, not a copy, so a
     * caller must not modify its bytes.
     * @param request - shell-carrier request for a `/plugins` resource.
     * @returns the exact response also exposed by the optional Web route.
     */
    fetchBundle(request: Request): Promise<Response>;
    /**
     * Filesystem baseline captured before an entry's current bytes were read.
     * HMR compares it with the live files when installing a watch, so a write
     * between startup composition and watch installation cannot disappear into
     * the watcher's initial state.
     * @param id - entry id (package name).
     * @returns the path and baseline, or undefined for an unknown id.
     */
    artifactBaseline(id: string): ClientArtifactBaseline | undefined;
    /**
     * Publish one completed bundle generation (the HMR watch's registration
     * hook; an entry restart publishes through reconciliation instead).
     * Unchanged mtime, ctime and size preserve the graph without reading the bundle.
     * @param id - entry id (package name).
     * @returns the current artifact rev, or undefined for an unknown id.
     */
    rebuilt(id: string): string | undefined;
    /**
     * Subscribe to bundle rebuilds; fires when {@link rebuilt} changes a rev. A rev an
     * entry restart publishes reaches only {@link onGraphChanged} listeners.
     * @param listener - receives the entry id and its new bundle rev.
     * @returns the unsubscriber.
     */
    onRebuilt(listener: (id: string, rev: string) => void): () => void;
    /**
     * Fires after any flush that recomposed the graph (row added/removed, or a
     * rebuilt rev change). Pull model: listeners re-read {@link graph}.
     * @param listener - notified with no payload.
     * @returns the unsubscriber.
     */
    onGraphChanged(listener: () => void): () => void;
    private compose;
    private notifyGraphChanged;
    private resolveMeta;
    /**
     * Locate the manifest of the package the Loader mounts for a row. The row's
     * module location is authoritative: the specifier resolves through the same
     * Loader resolution that imported the row's host half — including any
     * active ESM hooks — and the nearest ancestor manifest declaring the name
     * owns the module. Tree-anchored `require` resolution remains only for
     * runtimes without Node internals.
     * @param loaderName - module specifier of the loader row.
     * @param baseUrl - resolution base of the tree that owns the row.
     * @returns the manifest path, or `undefined` when the name resolves to no package root.
     */
    private locatePkgJson;
    private nearestPackage;
    private sourceKey;
    /** Capture the bundle stats before reading its bytes. */
    private captureArtifactBaseline;
    /**
     * Run one filesystem read of a declared client bundle.
     * @param pkgName - package that declares the client bundle.
     * @param clientPath - absolute path of the built client artifact.
     * @param read - the stat or read of `clientPath`.
     * @returns the read's result.
     * @throws {MissingClientBundleError} when the read fails with `ENOENT`; other filesystem errors are rethrown unchanged.
     */
    private readClientBundle;
    /** Treat a missing, torn, or malformed development map as an identity section. */
    private readonly readSourceMap;
    /**
     * Reconcile one entry name against its sources at flush start.
     * @param entryName - Loader entry name being reconciled.
     * @param entries - same-name entries collected at flush start.
     * @param onError - receives package reconciliation failures.
     * @returns whether the table changed.
     */
    private processOne;
    private resolveSource;
    private reconcilePackage;
    private flush;
    /** Match an exact current-revision package-local chunk URL without reading its file. */
    private chunkRequest;
    /** Build a package-local chunk response only when its URL is requested. */
    private chunkResponse;
    /**
     * Answer one package-asset request. Node's `require.resolve` applies the
     * package's `./assets/*` export as a self-reference, so unexported,
     * `.`/`..`, missing, and directory paths answer 404. The build writes each
     * asset's content revision into its file name, so a served path names one
     * content: the body streams from disk on every request, and the path is the
     * entity tag.
     * @param method - request method.
     * @param record - the active row of the addressed package, or undefined when it is not active.
     * @param assetPath - percent-encoded path below the package's `assets/` route segment.
     * @param ifNoneMatch - request `If-None-Match` field.
     * @returns the route answer.
     */
    private assetResponse;
    /**
     * Answer one `/plugins` request. A `/plugins/<package>/assets/` path is
     * answered by the package-asset route alone; any other path is an advertised
     * combo or its map, a prior startup generation, or a current-revision
     * package-local chunk, in that order, and otherwise 404.
     */
    private bundleResource;
}
export default ClientModuleRegistry;
//# sourceMappingURL=index.d.ts.map