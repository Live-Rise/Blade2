import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { HostObservable } from '@deepseek-ai/dsh-client-ui-slots';
import type { MainPanelId } from '@deepseek-ai/dsh-client-ui-layout/client';
import { type PluginManagerLocaleKey } from './locales.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** Read-only management activity, independent of the visible Plugins page. */
        pluginChanges: {
            /**
             * Observe whether a bundle or one of its rows is being changed.
             * @param packageName Bundle package name.
             * @returns A snapshot source whose subscriptions belong to the caller.
             */
            bundle(packageName: string): HostObservable<boolean>;
        };
        /** Cross-plugin navigation to the Plugins panel. */
        pluginNavigation: {
            /**
             * Open a bundle's details without changing the current Session.
             * An absent bundle displays the plugin list after loading.
             * @param packageName - npm package name of the bundle.
             */
            openBundle(packageName: string): void;
            /** Open the Plugins main list without opening a dialog or changing the current Session. */
            openList(): void;
        };
    }
}
export type { PluginManagerPageProps } from './PluginManagerPage.tsx';
export type { ConfigLedger, OfficialItem } from './config-ledger.ts';
export type { PluginManagerFace } from './manager-store.ts';
export type { PluginManagerLocaleKey } from './locales.ts';
export type { ConfigPageForm, PluginActivationOwnerProps, PluginAddActionsProps, PluginTabInstall, PluginTabProps, PluginConfigViewProps, PluginDetailProps, PluginPackageRef, PluginRowRef, PluginsSubject, } from './slot-contract.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Plugin manager tab copy. */
        'pluginManager': PluginManagerLocaleKey;
    }
}
/** Dictionary namespace owned by this plugin. */
export declare const NS = "pluginManager";
/** The id shared by the sidebar entry and the main panel it opens. */
export declare const PANEL_ID: MainPanelId;
/** Services required by the sidebar registration, the Remote methods, and access; the inventory says whether the Host manages a profile. */
export declare const inject: string[];
/**
 * Contribute the Plugins entry to the sidebar with the management page it
 * opens, and keep it current on the Host's change events, while the current
 * Peer may list the installed bundles.
 * @param ctx - the browser plugin context.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=index.d.ts.map