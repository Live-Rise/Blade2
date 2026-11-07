import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { LocalizedText } from '@deepseek-ai/dsh-package-manifest';
import { type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { HostObservable } from '@deepseek-ai/dsh-client-ui-slots';
import type { MarketIndexSummary, MarketSearchResult, PluginDetail, PluginDetailDocumentId } from '@deepseek-ai/dsh-api-remotes/client';
import type { LocaleSnapshot } from '@deepseek-ai/dsh-client-locale/client';
import { type DiscoverQuery } from './presentation.ts';
/** Why the last listing read failed: the index could not be fetched, is not a valid index, or changed while its pages loaded. */
export type ListingFailure = 'fetch' | 'invalid' | 'changed';
/** Why the last source change was refused. */
export type SourceFailure = {
    readonly kind: 'invalid' | 'duplicate' | 'unavailable';
} | {
    readonly kind: 'other';
    readonly reason: string;
};
/** Plugins loaded before "Show more", and loaded again by each press. */
export declare const PAGE_SIZE = 50;
/** The selected index's query and results. */
export interface ListingState {
    readonly url: string;
    readonly query: DiscoverQuery;
    /** `loading` while the first pages of a query or a reload run; `more` while the next page runs. */
    readonly status: 'loading' | 'more' | 'ready' | 'error';
    /**
     * Results of this URL: the last page's index facts with the entries of every loaded page. The previous results are
     * kept while a new query, reload, or refresh runs or fails.
     */
    readonly value: MarketSearchResult | null;
    /** The query `value` answers; it differs from `query` while a new query runs or after it failed. */
    readonly answered: DiscoverQuery | null;
    readonly failure: ListingFailure | null;
}
/** Shared Discover state. */
export interface DiscoverState {
    /** `idle` until the tab first renders. */
    readonly status: 'idle' | 'loading' | 'ready' | 'error';
    readonly indexes: readonly MarketIndexSummary[];
    readonly selected: string | null;
    readonly listing: ListingState | null;
    /** Whether the source editor is open; it is always shown when no source exists. */
    readonly editing: boolean;
    readonly draft: string;
    readonly saving: boolean;
    readonly sourceFailure: SourceFailure | null;
}
/** The market entry version a detail read is about. */
export interface DetailRequest {
    readonly url: string;
    readonly name: string;
    readonly version: string;
    /** Whether the version is an npm package; the Host reads those of the public registries from the npm file CDNs. */
    readonly npm: boolean;
}
/** Why a detail read failed: the Host's `plugin-market/detail-unavailable` reason, or `other` for any other failure. */
export type DetailFailureKind = 'too-large' | 'network' | 'denied' | 'git-unavailable' | 'invalid' | 'other';
/** One detail read. */
export type DetailState = {
    readonly status: 'loading';
} | {
    readonly status: 'ready';
    readonly detail: PluginDetail;
} | {
    readonly status: 'error';
    readonly failure: DetailFailureKind;
};
/** Detail reads by {@link detailKey}. */
export type DetailsState = Readonly<Record<string, DetailState>>;
/**
 * The identity of a detail read.
 * @param request - the plugin.
 * @param language - the display language.
 * @returns the key of the read in {@link DetailsState}.
 */
export declare function detailKey(request: DetailRequest, language: string): string;
/** What the entry detail section receives through injection. */
export interface DetailFace {
    readonly hooks: {
        /** Detail reads, bound as `useDetails`. */
        readonly details: SnapshotStore<DetailsState>;
        /** The active locale, bound as `useLocale`. */
        readonly locale: HostObservable<LocaleSnapshot>;
    };
    /** Read a plugin's detail document unless the read already ran or runs. */
    readonly loadDetail: (request: DetailRequest, language: string) => void;
    /** Read a failed detail document again, bypassing the Host's cached result. */
    readonly retryDetail: (request: DetailRequest, language: string) => void;
    /**
     * The URL of an image beside a detail document.
     * @returns the authenticated Host URL, or undefined for an absolute, protocol-relative, or fragment destination.
     */
    readonly detailImage: (document: PluginDetailDocumentId, destination: string) => string | undefined;
}
/** What the Discover tab receives through injection. */
export interface DiscoverFace extends DetailFace {
    readonly hooks: DetailFace['hooks'] & {
        /** Discover state, bound as `useDiscover`. */
        readonly discover: SnapshotStore<DiscoverState>;
    };
    /** Resolve market text in the current language. */
    readonly resolveText: (text: LocalizedText) => string;
    /** Read the sources once the tab first renders. */
    readonly ensure: () => void;
    /** Read the source list again after it failed. */
    readonly reload: () => void;
    readonly select: (url: string) => void;
    /** Replace the selected index's query and load its first page. */
    readonly search: (query: DiscoverQuery) => void;
    /** Load the next page of the current results. */
    readonly more: () => void;
    /** Fetch the selected index again. */
    readonly refresh: () => void;
    /** Run the last failed listing read again. */
    readonly retry: () => void;
    readonly toggleSources: () => void;
    readonly editDraft: (text: string) => void;
    readonly addSource: () => void;
    readonly removeSource: (url: string) => void;
}
/** Owns Discover reads and source edits for one Client. */
export declare class DiscoverController {
    private readonly ctx;
    private readonly base;
    /** Shared Discover state published to the tab. */
    readonly store: SnapshotStore<DiscoverState>;
    /** Detail reads published to detail sections. */
    readonly details: SnapshotStore<DetailsState>;
    private disposed;
    /** The latest detail read of each key; an older read's answer is dropped. */
    private readonly detailReads;
    /** Numbers detail reads across the controller's lifetime, so a read started before a reset never matches one after it. */
    private detailSequence;
    private indexRead;
    private listingRead;
    /** Select the last source once the list it was added to arrives. */
    private selectAdded;
    /** Runs the listing read that failed last. */
    private failed;
    /**
     * @param ctx - the plugin context whose `remote.pluginMarket` namespace answers.
     * @param base - the application base URL that Host routes resolve against.
     */
    constructor(ctx: ClientContext, base: () => string);
    /** Stop publishing late settlements. */
    dispose(): void;
    /**
     * Build the tab's injected face.
     * @param resolveText - current-language text resolution from the locale service.
     * @returns the tab's injected state sources and actions.
     */
    face(resolveText: DiscoverFace['resolveText']): DiscoverFace;
    /** Detail reads and image addressing of the entry detail section. */
    private detailFace;
    /**
     * Forget every detail read after the connection reset: a restarted Host no longer knows the document ids that address
     * their images, and each shown section reads its document again.
     */
    detailsReset(): void;
    /** Re-read the sources after the Host announced a change or the connection reset; an unrendered tab stays idle. */
    sourcesChanged(): void;
    /** Recompute installation status of every loaded plugin after the profile's bundles changed; the Host reuses its cached document. */
    bundlesChanged(): void;
    private readDetail;
    private publishDetail;
    private state;
    private patch;
    private loadIndexes;
    /**
     * Load the first `count` results of a query in pages the Host accepts, replacing the shown results at once. Later
     * pages pin the first page's document; when the Host answers from a replaced document the read restarts once, and
     * fails if the document is replaced again. The index's undeclared ids leave the query.
     */
    private load;
    /** Load the page after the shown results; a replaced document reloads them from the first page. */
    private loadMore;
    private page;
    /** Publish a settled read and the index title or error it taught about its source. */
    private settle;
    private saveSources;
}
//# sourceMappingURL=market-store.d.ts.map