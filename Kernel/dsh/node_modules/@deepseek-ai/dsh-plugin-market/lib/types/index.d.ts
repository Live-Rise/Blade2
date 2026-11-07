/** Host reader of configured plugin market indexes for the Discover page and the `plugin_market` tool. */
import type { Context, Volatile } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { NpmFileSource } from '@deepseek-ai/dsh-plugin-detail/types';
import type { MarketIndexSummary, MarketListingEntry, MarketQuery, MarketSearchResult, PluginDetail } from './types.ts';
export type * from './types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** Configured plugin market index sources and their validated listings. */
        pluginMarket: PluginMarket;
    }
}
/** Index sources and fetch limits. */
export interface Config {
    /** Index URLs in display order; Settings and `setIndexes` write this field without remounting the service. */
    indexes: Volatile<string[]>;
    /** Deadline for one index fetch, including the body. */
    fetchTimeoutMs: number;
    /** Largest accepted index body in bytes. */
    maxBytes: number;
    /** Lifetime of a successfully read index document, a detail read result, and a repository's default-branch commit. */
    cacheTtlMs: number;
    /** Largest detail document in bytes. */
    detailMaxBytes: number;
    /** Largest image beside a detail document in bytes. */
    detailImageMaxBytes: number;
    /** Largest amount in bytes one Git command adds to a partial clone; a repository whose fetch adds more is not read. */
    detailRepositoryMaxBytes: number;
    /** Largest total size of the partial clones; least recently used ones are deleted beyond it. */
    detailCacheMaxBytes: number;
    /** Directory of the partial clones, relative to the profile directory; absent means `.plugin-market/repositories`. */
    detailCacheDir?: string;
    /** Deadline of one Git command of the clone fallback. */
    detailGitTimeoutMs: number;
    /** npm file CDNs read in order; a read's `npmFileSources` moves the CDNs it names first. */
    detailNpmFileSources: NpmFileSource[];
    /** Hosts serving the GitLab repository files API. */
    detailGitlabHosts: string[];
}
/** Fetches, validates, and caches configured market indexes; annotates entries with the profile's installation facts. */
export default class PluginMarket extends TypertRemoteService {
    private readonly config;
    static inject: string[];
    static Config: z<Schemastery.ObjectS<NoInfer<{
        indexes: z<NoInfer<string[]>, NoInfer<string[]>, "volatile-defined">;
        fetchTimeoutMs: z<number, number, "defined">;
        maxBytes: z<number, number, "defined">;
        cacheTtlMs: z<number, number, "defined">;
        detailMaxBytes: z<number, number, "defined">;
        detailImageMaxBytes: z<number, number, "defined">;
        detailRepositoryMaxBytes: z<number, number, "defined">;
        detailCacheMaxBytes: z<number, number, "defined">;
        detailCacheDir: z<string, string, "plain">;
        detailGitTimeoutMs: z<number, number, "defined">;
        detailNpmFileSources: z<("jsdelivr" | "unpkg" | "npmmirror")[], ("jsdelivr" | "unpkg" | "npmmirror")[], "defined">;
        detailGitlabHosts: z<string[], string[], "defined">;
    }>>, Schemastery.ObjectT<NoInfer<{
        indexes: z<NoInfer<string[]>, NoInfer<string[]>, "volatile-defined">;
        fetchTimeoutMs: z<number, number, "defined">;
        maxBytes: z<number, number, "defined">;
        cacheTtlMs: z<number, number, "defined">;
        detailMaxBytes: z<number, number, "defined">;
        detailImageMaxBytes: z<number, number, "defined">;
        detailRepositoryMaxBytes: z<number, number, "defined">;
        detailCacheMaxBytes: z<number, number, "defined">;
        detailCacheDir: z<string, string, "plain">;
        detailGitTimeoutMs: z<number, number, "defined">;
        detailNpmFileSources: z<("jsdelivr" | "unpkg" | "npmmirror")[], ("jsdelivr" | "unpkg" | "npmmirror")[], "defined">;
        detailGitlabHosts: z<string[], string[], "defined">;
    }>>, "plain">;
    private readonly lifetime;
    private readonly cache;
    private readonly pending;
    private readonly errors;
    private readonly entryId;
    private readonly repositories;
    private readonly details;
    constructor(ctx: Context, config: Config);
    /**
     * List the configured index sources without fetching them.
     * @returns every source in order, with the title of its last successful read and, when the last read failed, its error.
     */
    listIndexes(): MarketIndexSummary[];
    /**
     * Replace the configured index sources through Settings, without remounting this service.
     * `plugin-market/changed` announces the committed list. A URL already configured is kept as written, so a client can
     * add or remove sources beside one that `listIndexes` reports as unusable.
     * @param urls - index URLs in display order.
     * @throws {RemoteError} `plugin-market/invalid-index-url` for an invalid new URL or a repeated URL;
     * `plugin-market/settings-unavailable` without Settings or a profile entry.
     */
    setIndexes(urls: string[]): Promise<void>;
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
    search(url: string, query: MarketQuery): Promise<MarketSearchResult>;
    /**
     * Read one plugin of a configured index, using the cached document as `search` does.
     * @param url - a configured index URL.
     * @param name - the plugin's package name.
     * @returns the plugin with its default version, installation status, and annotated versions; undefined when the
     *   index does not list it.
     * @throws {RemoteError} `plugin-market/unknown-index`, `plugin-market/invalid-index-url`, `plugin-market/fetch-failed`,
     *   or `plugin-market/invalid-index`.
     */
    entry(url: string, name: string): Promise<MarketListingEntry | undefined>;
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
    readEntryDetail(url: string, name: string, version: string, language: string, options?: {
        refresh?: boolean;
        npmFileSources?: NpmFileSource[];
    }): Promise<PluginDetail>;
    private languages;
    private detailRepositoryOptions;
    /** The detail location of a published npm version on the configured CDNs, the preferred ones first. */
    private npmLocation;
    private readDetail;
    /** Answer `GET ${DETAIL_FILE_PATH}?document=<id>&path=<destination>` with one image beside a read detail document. */
    private serveDetailFile;
    private document;
    /** Annotate plugins with the profile's bundles and exemptions as they are now. */
    private annotator;
    /**
     * The configured sources as written, each with the refusal that makes it unusable. Settings and profile files can
     * write any http(s) string, so every use validates instead of the schema; an unusable source is listed with its error
     * and never fetched, and a repeated URL counts once.
     */
    private sources;
    private configured;
    private fetchShared;
    private fetchIndex;
    /** Read the whole body, refusing it as soon as it exceeds `maxBytes`. */
    private readBody;
}
//# sourceMappingURL=index.d.ts.map