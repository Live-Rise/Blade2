/**
 * The web search and fetch settings page, browser half: one provider menu per
 * capability, which writes the `web` provider selection, the Search API result
 * count over the `web-deepseek-api` namespace, and legacy Messages search's
 * key, endpoint, and search limit over `web-search-deepseek`. The page
 * registers into the Plugins page's `plugins.item` slot while the Host serves
 * `web`; a provider's own controls appear only while the Host serves its
 * namespace.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import { type WebSearchSettingsLocaleKey } from './locales.ts';
export type { WebSearchCardProps } from './WebSearchCard.tsx';
export type { WebApiSettings, WebLegacySettings, WebProviderSelection, WebSearchCardFace, WebSearchCardScopes, WebSearchCardState, } from './web-search-card-controller.ts';
export type { WebSearchSettingsLocaleKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Web search and fetch settings page copy. */
        'settings.webSearch': WebSearchSettingsLocaleKey;
    }
}
/** Dictionary namespace owned by this plugin. */
export declare const NS = "settings.webSearch";
/** Required services (cordis fiber inject). */
export declare const inject: string[];
/**
 * Mount the web search and fetch settings page while the Host serves the `web` namespace.
 * @param ctx - the browser plugin context.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=index.d.ts.map