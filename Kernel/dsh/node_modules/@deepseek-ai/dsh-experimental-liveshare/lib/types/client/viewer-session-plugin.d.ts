/**
 * Viewer-local plugin that reads the share the page presents and opens its one shared Session.
 *
 * The Liveshare client face mounts this plugin only on a viewer page, in place of the owner's Share
 * action. The share's own answer — who owns it and what mode it grants this page — is read here and
 * states the page's condition; which writes each Session offers comes from the calling Peer's access
 * snapshot, which every surface reads through its own access Hook.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-session-plugin
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
export declare const name = "liveshare-viewer-session";
/**
 * The session list says when this page's Session exists, Workspace navigation selects it, the access
 * snapshot says which Sessions this page may prompt, and the Connection says whether reads reach the
 * owner.
 */
export declare const inject: readonly string[];
/**
 * The Relay's sign-in entry, asked to return the reader to the page they left.
 *
 * A viewer page names its share in the query (`?shareId=...`), so the return address keeps the query
 * as well as the path: a sign-in that returned to the bare path would drop the share the reader is
 * signing in for. The fragment never leaves the page — a single-page app may keep a one-time
 * hand-off there — and the Relay accepts only a relay-relative path as a return address.
 * @param origin - Origin the Relay is served on.
 * @param basePath - Relay base path this page is served under.
 * @param page - The page's own path and query.
 * @returns The absolute sign-in entry carrying the page's return address.
 */
export declare function viewerSignInHref(origin: string, basePath: string, page: {
    readonly pathname: string;
    readonly search: string;
}): string;
/**
 * Read the share, open the Session the served document names, and keep every composer honest.
 *
 * A viewer may read the shared Session and nothing else unless the owner grants more. The
 * framework's own composer block is the way to say that: while the access snapshot excludes
 * `session/prompt` for a Session, the block disables the input and renders the page's reason, and the
 * composer itself withholds Send and Stop, so the read-only transcript keeps streaming without a send
 * affordance. Page disposal cancels and awaits bootstrap, focus, and retry reads; late answers cannot
 * publish.
 * @param ctx - browser context of the viewer's module table.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=viewer-session-plugin.d.ts.map