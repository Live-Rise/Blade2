import type { SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { LocalizedText } from '@deepseek-ai/dsh-package-manifest';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { ToolCallId } from '@deepseek-ai/dsh-llm/brand';
import type { ActionCardBodyProps } from '@deepseek-ai/dsh-client-ui-action-cards/client';
import type { RecommendationDetails } from '@deepseek-ai/dsh-plugin-recommendations/types';
/** Catalog text the Host returned for an available plugin. */
export type CatalogDetails = Extract<RecommendationDetails, {
    readonly status: 'available';
}>;
/** Plugin-lifetime services the body uses besides the frame's props. */
export interface PluginCardServices {
    /** Catalog text by plugin name, kept for this plugin's lifetime so later cards of the same plugin render it at once. */
    readonly catalog: SnapshotStore<Readonly<Record<string, CatalogDetails>>>;
    /**
     * Read the current catalog text of a card's plugin.
     * @param sessionId - the displayed Session.
     * @param callId - the card's call id.
     * @returns the Host's answer, or undefined when the reply was lost.
     */
    readonly describe: (sessionId: SessionId, callId: ToolCallId) => Promise<RecommendationDetails | undefined>;
    /** Open the plugin's details page without enabling it. */
    readonly openDetails: (packageName: string) => void;
    /** Pick the interface language's text. */
    readonly resolveText: (text: LocalizedText) => string;
}
/** Body props: the frame's card props plus the plugin-lifetime services. */
export type PluginCardProps = ActionCardBodyProps<'pluginRecommendations'> & PluginCardServices;
/**
 * Display one plugin card: its current catalog title and description, the user's choice and its outcome.
 * Until the plugin's first catalog lookup returns, a placeholder line and the disabled actions hold the final layout.
 * A refused or lost lookup leaves Not now and a retry of the lookup, but not Enable.
 * @param props - card, state, actions and plugin-lifetime services.
 * @returns the card body.
 */
export declare function PluginCardBody({ proposal, decision, sessionId, summary, act, catalog, describe, openDetails, resolveText, t, }: PluginCardProps): import("react").JSX.Element;
//# sourceMappingURL=Card.d.ts.map