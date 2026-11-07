/** Bundle detail reads: the long description of each bundle the page opens, read from the Host by name and language. */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import { type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { BundleDetail } from '@deepseek-ai/dsh-api-remotes/client';
/** Why a bundle detail read failed: the Host's `plugin-manager/detail-unavailable` reason, or `other`. */
export type BundleDetailFailure = 'too-large' | 'network' | 'denied' | 'git-unavailable' | 'invalid' | 'other';
/** One bundle detail read. */
export type BundleDetailState = {
    readonly status: 'loading';
} | {
    readonly status: 'ready';
    readonly detail: BundleDetail;
} | {
    readonly status: 'error';
    readonly failure: BundleDetailFailure;
};
/** Bundle detail reads by {@link bundleDetailKey}. */
export type BundleDetailsState = Readonly<Record<string, BundleDetailState>>;
/**
 * The identity of a bundle detail read: the bundle, its installed version, and the display language.
 * @param name - the bundle's package name.
 * @param version - the installed or offered version, so an updated bundle reads again.
 * @param language - the display language.
 * @returns the key of the read in {@link BundleDetailsState}.
 */
export declare function bundleDetailKey(name: string, version: string | undefined, language: string): string;
/** Reads bundle detail documents and publishes them to the page. */
export declare class BundleDetailController {
    private readonly ctx;
    private readonly base;
    /** Detail reads published to the page. */
    readonly store: SnapshotStore<BundleDetailsState>;
    /** The latest read of each key, numbered across the controller's lifetime; an older read's answer is dropped. */
    private readonly reads;
    private sequence;
    /** npm file CDNs to ask first, from the Host's latest registry comparison: npmmirror when its registry answered fastest. */
    private npmFileSources;
    private disposed;
    /**
     * @param ctx - the page plugin's context, whose `remote.pluginManager` namespace answers.
     * @param base - the application base URL that Host routes resolve against.
     */
    constructor(ctx: ClientContext, base: () => string);
    /**
     * Read a bundle's document unless the read already ran or runs.
     * @param name - the bundle's package name.
     * @param version - its installed or offered version.
     * @param language - the display language.
     * @param fromNpm - whether the bundle's files are unavailable, so the Host reads its offered version from the npm file CDNs.
     */
    load(name: string, version: string | undefined, language: string, fromNpm: boolean): void;
    /**
     * Read a bundle's document again.
     * @param name - the bundle's package name.
     * @param version - its installed or offered version.
     * @param language - the display language.
     * @param fromNpm - whether the Host reads the bundle from the npm file CDNs.
     */
    retry(name: string, version: string | undefined, language: string, fromNpm: boolean): void;
    /**
     * The URL of an image beside a bundle's document.
     * @param name - the bundle's package name.
     * @param document - the document's path inside the bundle.
     * @param destination - the image destination as the document writes it.
     * @param fromNpm - whether the Host reads the bundle from the npm file CDNs.
     * @returns the authenticated Host URL, or undefined for an absolute, protocol-relative, or fragment destination.
     */
    image(name: string, document: string, destination: string, fromNpm: boolean): string | undefined;
    /** Invalidate reads while retaining settled documents until their replacements arrive. */
    reset(): void;
    /** Stop publishing late settlements. */
    dispose(): void;
    private read;
    private publish;
}
//# sourceMappingURL=detail-store.d.ts.map