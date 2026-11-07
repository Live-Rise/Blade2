/**
 * The reader page's own answer for a workspace-file address a share cannot open.
 *
 * The viewer renders the shipped right Sidebar, which resolves every address through its tab-type
 * registry and refuses one no type claims. A share hands over workspace paths, not file contents, so
 * this page claims `dsh-resource://file/**` itself and says so in the tab: the address is answered by
 * the page that owns the fact, not by Chat's call site.
 *
 * The Liveshare client face mounts this module only on a viewer page.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-share-path
 */
import type { ReactNode } from 'react';
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
export declare const name = "liveshare-viewer-share-path";
/** The tab-type registry and the slot ledger; the parent face registers this page's copy. */
export declare const inject: readonly string[];
/** The body's composed props: the tab it draws and this page's copy. */
export type ViewerSharePathBodyProps = PropsRuntime<'sidebar.right.pane.tab'> & PropsLocale<'liveshare'>;
/**
 * State why this page cannot show the file the reader asked for.
 * @param props - the tab's framework share and this page's locale seat.
 * @returns the notice body.
 */
export declare function ViewerSharePathBody({ useTabInfo, t }: ViewerSharePathBodyProps): ReactNode;
/**
 * Claim workspace-file addresses on the reader's page and answer them in place.
 * @param ctx - browser context of the viewer's module table.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=viewer-share-path.d.ts.map