import type { Context } from '@deepseek-ai/cordis';
import type { TypertRemoteContribution } from '@deepseek-ai/dsh-typert-protocol';
import { type RecommendationKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        pluginRecommendations: RecommendationKey;
    }
}
/** Services the body needs; the shared card plugin provides `actionCards`. */
export declare const inject: string[];
/** Mount the catalog-text Remote, then register the body for the same plugin lifetime.
 * @param ctx - Client plugin context.
 * @param contribution - Generated Remote descriptors supplied by the built entry.
 * @returns disposer removing the body before the Remote contribution.
 */
export declare function mountRecommendations(ctx: Context, contribution: TypertRemoteContribution): Promise<() => Promise<void>>;
//# sourceMappingURL=mount.d.ts.map