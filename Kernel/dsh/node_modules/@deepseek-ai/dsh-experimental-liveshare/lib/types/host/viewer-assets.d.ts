/**
 * Owner-side resources of the served viewer page: the rendered index, shell assets, and client
 * plugin bundles, read for a viewer the Relay already authenticated.
 *
 * The page is the owner's own Web shell, so every byte comes from the services that serve the owner's
 * page: the webserver's resource read for the index and shell assets, and the client module registry for
 * plugin bundles. The resource read receives the reader's own Peer, so the index renders that reader's
 * opening access snapshot and the page offers only what it may call from its first render.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-assets
 */
import type { Context } from '@deepseek-ai/cordis';
import type { PeerScope } from '@deepseek-ai/dsh-typert-protocol';
import type { ViewerAnswer } from '../viewer-channel.ts';
/**
 * Read one served-page resource.
 * @param ctx - Owner Host context carrying the webserver and the client module registry.
 * @param path - Validated page-relative path and query: the index, `assets/...`, or `plugins/...`.
 * @param peer - The reader's own Peer; the page resource renders the opening access rows for it.
 * @returns The answer the Relay serves for that path.
 */
export declare function readViewerAsset(ctx: Context, path: string, peer: PeerScope): Promise<ViewerAnswer>;
//# sourceMappingURL=viewer-assets.d.ts.map