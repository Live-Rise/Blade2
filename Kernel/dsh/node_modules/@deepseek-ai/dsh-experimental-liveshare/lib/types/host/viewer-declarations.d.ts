/**
 * Temporary: the Remote endpoints plugins offer to share viewers.
 *
 * A plugin that owns an endpoint the served viewer page needs declares it here, because the `share-viewer`
 * policy decides every viewer call and enumerates only the core endpoints. **A declaration is an offer, not
 * a grant**: it says the plugin asserts the endpoint may be called from a share, and grants nothing by
 * itself. The owner exposes it by selecting that plugin's endpoint in one share, and only the intersection
 * of the live declarations with one share's own selection reaches that share's viewers.
 *
 * This surface is **temporary**: it is replaced by endpoint-level access declarations in Typert metadata,
 * and it is deleted once that metadata names each endpoint's access class; do not build a long-term
 * integration on it.
 *
 * The service carries no `Context` key of its own, so a plugin reads it through
 * {@link viewerDeclarationsOf}, which preserves the calling context; the lookup is untyped by
 * construction, so the accessor narrows the value it finds.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-declarations
 */
import { Service, type Context } from '@deepseek-ai/cordis';
import type { RemoteAccessEndpoint } from '@deepseek-ai/dsh-api-remote-access/types';
/**
 * Name the capability service is provided under. Deliberately not a `Context` key: a key would enter the
 * generated Cordis API catalog, whose partitions this change must not touch.
 */
export declare const VIEWER_DECLARATIONS_SERVICE = "liveshareViewerDeclarations";
/** One plugin's endpoints of each kind, in the shape the share panel and the share record both use. */
export interface ViewerCapabilityGroup {
    /** Owning npm package of the declaring plugin, shown beside each choice. */
    readonly plugin: string;
    /** Readable endpoints of that plugin, sorted. */
    readonly readable: readonly RemoteAccessEndpoint[];
    /** Writable endpoints of that plugin, sorted. */
    readonly writable: readonly RemoteAccessEndpoint[];
}
/**
 * The declaration surface as a consumer reads it: the operations a provider must offer.
 *
 * Consumers narrow the service values they find structurally rather than by `ViewerDeclarations` class
 * identity, because the Loader instantiates a module once per entry: the entry that provides the surface and
 * the entry that mounts the share host hold different classes for the same value, and `instanceof` refuses
 * the provider's own instance.
 */
export interface ViewerDeclarationsSurface {
    /** Offer endpoints that change nothing; see {@link ViewerDeclarations.registerViewerReadable}. */
    registerViewerReadable(actions: readonly string[]): () => void;
    /** Offer endpoints that write; see {@link ViewerDeclarations.registerViewerWritable}. */
    registerViewerWritable(actions: readonly string[]): () => void;
    /** Observe every change to the offered set; see {@link ViewerDeclarations.onChanged}. */
    onChanged(listener: () => void): () => void;
    /** The endpoints plugins currently offer; see {@link ViewerDeclarations.declaredCapabilities}. */
    declaredCapabilities(): readonly ViewerCapabilityGroup[];
}
/**
 * Temporary: the endpoints plugins offer to share viewers.
 *
 * A declaration is checked before it is accepted: every endpoint must be a live Host Remote definition
 * owned by the declaring plugin's own package (see {@link declaringPackage}). The declaring plugin owns
 * the endpoint's confinement, because a declared endpoint's arguments arrive from an untrusted viewer; no
 * argument may be treated as authority.
 *
 * A declaration is an offer: the owner exposes an offered endpoint per share, and a declaration on its own
 * grants no viewer anything. Registration is a Cordis effect of the CALLER's fiber: read the service
 * through {@link viewerDeclarationsOf} so that Cordis traces the calling context. Disposing the declaring
 * plugin, or calling the returned disposer, withdraws exactly the names that call registered, and a share
 * whose owner selected one of them stops exposing it on the next decision. A name declared more than once
 * stays offered until the last registration naming it is disposed.
 *
 * The Liveshare bundle provides this service in its own load phase, before the share host — which waits for
 * a Relay connection — starts, so a plugin that offers endpoints at its own load time finds it. The share
 * host is one consumer: it reads the same service and subscribes through {@link onChanged}.
 *
 * The endpoints a declaration names are the Host's local Typert registrations. typert-loader registers a
 * package's artifact asynchronously, so an offer made from the package's own load body runs before its
 * endpoints exist; a plugin in that position must declare once its package's artifact is registered.
 */
export declare class ViewerDeclarations extends Service implements ViewerDeclarationsSurface {
    /** Live readable declarations: endpoint → owning package → live registrations. */
    private readonly readable;
    /** Live writable declarations, keyed the same way. */
    private readonly writable;
    /** Live change listeners, called after every registration and disposal. */
    private readonly changed;
    /**
     * Provide the capability surface on `ctx`.
     * @param ctx - Context that provides the service; the service goes away with its fiber.
     */
    constructor(ctx: Context);
    /**
     * Observe every change to the offered set.
     *
     * A consumer that derives decisions from {@link declaredCapabilities} recomputes them here; a listener
     * registered after a declaration still receives every later change. Bind the returned disposer to the
     * subscriber's own fiber so the listener goes away with it.
     * @param listener - Called after each registration and disposal that changed the offered set.
     * @returns Disposer removing this listener.
     */
    onChanged(listener: () => void): () => void;
    /** Tell every live listener that the offered set may have changed. */
    private notifyChanged;
    /**
     * Temporary: lets a plugin declare endpoints it asserts are read-only and safe to expose to any
     * authenticated share viewer. Superseded by endpoint-level access declarations in Typert metadata,
     * and this surface is deleted once they land; do not build a long-term integration on it.
     *
     * The assertion covers three obligations: the endpoint changes no Host state; it starts no work, sends
     * nothing, and schedules nothing; and any authenticated share viewer may call it on the resources the
     * share exposes. **This call is an offer and grants nothing**: the owner must select the endpoint in one
     * share before that share's viewers may call it.
     * @param actions - Endpoint names, each `<namespace>/<method>`.
     * @returns Disposer withdrawing the names this call registered.
     * @throws {TypeError} When a name is not a `<namespace>/<method>` Remote endpoint, the Host does not
     *   define it, or the declaring package does not own it; no name registers.
     */
    registerViewerReadable(actions: readonly string[]): () => void;
    /**
     * Temporary: lets a plugin declare endpoints it asserts are safe to invoke from a share **whose session
     * mode already allows writes**. Superseded by endpoint-level access declarations in Typert metadata,
     * and this surface is deleted once they land; do not build a long-term integration on it.
     *
     * **This call is an offer and grants nothing, and it never escalates a read-only share.** A writable
     * endpoint is exposed only when the owner also selects it for that share, and the owner states consent
     * for writes only through the share's session mode, so a `view` share refuses it
     * whether it is declared and selected or not. A declared write endpoint runs on the owner's machine, so
     * the plugin owns its confinement.
     * @param actions - Endpoint names, each `<namespace>/<method>`.
     * @returns Disposer withdrawing the names this call registered.
     * @throws {TypeError} When a name is not a `<namespace>/<method>` Remote endpoint, the Host does not
     *   define it, or the declaring package does not own it; no name registers.
     */
    registerViewerWritable(actions: readonly string[]): () => void;
    /**
     * The endpoints plugins currently offer, grouped by owning package and sorted.
     *
     * This is the offer list the share panel presents; it is not what any share exposes.
     * @returns One group per owning package with at least one live declaration.
     */
    declaredCapabilities(): readonly ViewerCapabilityGroup[];
    /**
     * Register one declaration kind as an effect of `caller`, the declaring plugin's own context.
     * @param declared - Live declarations of that kind.
     * @param caller - Context the effect binds to; the caller's context, not the share host's.
     * @param kind - Endpoint kind the declaration offers.
     * @param actions - Endpoint names the declaration carries.
     * @param label - Declaration kind named in the failure message.
     * @returns Disposer withdrawing the names this call registered.
     * @throws {TypeError} When a name is not a `<namespace>/<method>` Remote endpoint, the Host does not
     *   define it, or the declaring package does not own it; no name registers.
     */
    private declare;
}
/**
 * Temporary: the capability surface of an application that mounts the Liveshare bundle.
 *
 * The lookup is deliberately untyped — the service declares no `Context` key, because such a key would
 * have to enter the generated Cordis API catalog — so this accessor is the single narrowing point. It
 * accepts any value that carries the whole {@link ViewerDeclarationsSurface}, so a provider reached through
 * another Loader entry is read as the surface it is.
 * @param ctx - Context in the application that may mount the bundle.
 * @returns The capability surface, or undefined when this application mounts no Liveshare bundle.
 */
export declare function viewerDeclarationsOf(ctx: Context): ViewerDeclarationsSurface | undefined;
//# sourceMappingURL=viewer-declarations.d.ts.map