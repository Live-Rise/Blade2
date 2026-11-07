import { Service, type Context } from '@deepseek-ai/cordis';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { ToolCallId } from '@deepseek-ai/dsh-llm';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { RecommendationDetails } from './types.ts';
export type * from './types.ts';
export declare const name = "plugin-recommendations";
/** Services the plugin uses; `actionCards` owns the card tool, card state and user actions. */
export declare const inject: string[];
declare module '@deepseek-ai/cordis' {
    interface Context {
        pluginRecommendations: PluginRecommendations;
    }
}
/** Catalog lookups for card text and the enabling behind the `enable_official_plugin` card. */
export declare class PluginRecommendations extends TypertRemoteService {
    static inject: string[];
    /** Card reconciliations and enabling in flight; unloading waits for them. */
    private readonly operations;
    private readonly stopped;
    private closing;
    private readonly owner;
    constructor(ctx: Context);
    protected [Service.init](): void;
    /**
     * Read the current catalog text for a logged card.
     * @param agent - Exact live root resolved by the Gateway from the Session id.
     * @param callId - The card's `propose_action` call id, never a package supplied by the browser.
     * @returns the plugin's title, description and selection, or a refusal when the card names no available official plugin.
     */
    describe(agent: Agent, callId: ToolCallId): Promise<RecommendationDetails>;
    private isStopping;
    /** Track an operation until it settles, so unloading waits for it. */
    private tracked;
    /** Record `enabled` on an unconfirmed card whose interrupted enable has since turned its plugin on. */
    private confirm;
    /** Enable the card's plugin once; an enabled card settles unchanged, and a failed or unconfirmed one can be retried. */
    private enable;
}
/**
 * Register the official-plugins skill, the `enable_official_plugin` card kind and the catalog-text Remote;
 * disposing the plugin stops local follow-up of pending enabling and removes all three.
 * @param ctx - Owning plugin context.
 */
export declare function apply(ctx: Context): Promise<void>;
//# sourceMappingURL=index.d.ts.map