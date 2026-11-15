/**
 * The web search and fetch settings page: one section per capability, each
 * with a provider menu and its explanation. The selected provider's settings
 * below the menu sit in one gray panel: the result count and billing for the
 * DeepSeek Search API, billing for the DeepSeek Fetch API, and the key,
 * endpoint, and search limit for legacy Messages search. A provider with no
 * settings here shows no panel.
 */
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import { type WebSearchCardFace } from './web-search-card-controller.ts';
/** Props the renderer binds for the web search and fetch page. */
export type WebSearchCardProps = PropsRuntime<'plugins.item'> & PropsLocale<'settings.webSearch'> & InjectFace<WebSearchCardFace>;
/**
 * Render the page's one-liner or its settings form, as the Plugins page asks.
 * @param props - the view asked for, locale copy, the form snapshot, and its actions.
 * @returns the one-liner, or the form.
 */
export declare function WebSearchCard(props: WebSearchCardProps): string | import("react").JSX.Element;
//# sourceMappingURL=WebSearchCard.d.ts.map