/**
 * The web-search settings page, browser half: the switch between DeepSeek
 * Messages search and the Search and Fetch APIs, which writes the `web`
 * provider selection, and the selected API's key, endpoint, and per-search
 * limit over the `web-search-deepseek` or `web-deepseek-api` namespace. The
 * page registers into the Plugins page's `plugins.item` slot while the Host
 * serves `web-search-deepseek`, so a deployment without that provider shows no
 * trace of it; the switch appears only while the Host also serves `web` and
 * `web-deepseek-api`.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import { type WebSearchSettingsLocaleKey } from './locales.ts';
export type { WebSearchCardProps } from './WebSearchCard.tsx';
export type { WebApiSettings, WebProviderSelection, WebSearchCardFace, WebSearchCardScopes, WebSearchCardState, WebSearchSettings, } from './web-search-card-controller.ts';
export type { WebSearchSettingsLocaleKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Web-search settings page copy. */
        'settings.webSearch': WebSearchSettingsLocaleKey;
    }
}
/** Dictionary namespace owned by this plugin. */
export declare const NS = "settings.webSearch";
/** Required services (cordis fiber inject). */
export declare const inject: string[];
/**
 * Mount the web-search settings page while the Host serves its namespace.
 * @param ctx - the browser plugin context.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=index.d.ts.map