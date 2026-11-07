import type { Context } from '@deepseek-ai/cordis';
import { type RecommendationKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        creationRecommendations: RecommendationKey;
    }
}
/** Services the body registration needs; the shared card plugin provides `actionCards`. */
export declare const inject: string[];
/**
 * Register the `create_personalized_page` body for the plugin lifetime.
 * @param ctx - Client plugin context.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=mount.d.ts.map