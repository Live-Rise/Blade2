/**
 * Browser plugin: the owner's Session-scoped Liveshare action, or the viewer page's own surfaces.
 *
 * The served viewer page is the owner's own Web shell with every owner plugin, so this one face
 * decides which half it mounts from the page global the Relay's carrier installs
 * (`__DSH_LIVESHARE_VIEWER__`): absent on the owner's page, present on a viewer page.
 *
 * The owner half mounts the generated `liveshareShares` Remote before it contributes the header
 * action, because that action and its dialogs address every share-management call through it.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import { type LiveshareKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        liveshare: LiveshareKey;
    }
}
/** Required client services: dictionaries, the Remote carrier, and the header contribution. */
export declare const inject: string[];
/**
 * Register Liveshare dictionaries and the shared-session origin seat, then the owner's Session
 * Header utility or the viewer surfaces.
 *
 * The owner Remote is mounted only on the owner page: a viewer Peer is refused every management
 * method, so a viewer page has nothing to mount it for.
 * @param ctx - browser context carrying slot, locale, and Remote services.
 * @returns Disposer releasing the owner Remote when the face unmounts.
 */
export declare function apply(ctx: ClientContext): Promise<() => Promise<void>>;
export type { LiveshareShareActionProps } from './ShareAction.tsx';
export type { LiveshareShareDialogProps } from './ShareDialog.tsx';
export type { LiveshareManageSharesDialogProps } from './ManageSharesDialog.tsx';
//# sourceMappingURL=index.d.ts.map