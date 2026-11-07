import type { Context } from '@deepseek-ai/cordis';
import { HarnessError } from '@deepseek-ai/dsh-llm';
export type * from './types.ts';
export declare const name = "creation-recommendations";
/** Services the page kind uses; `actionCards` owns the card tool, state and user actions. */
export declare const inject: string[];
/** Invalid generated output never changes the profile. */
export declare class CreationRecommendationError extends HarnessError {
}
/**
 * Register the page planning skill and the `create_personalized_page` card kind; disposing the plugin stops owned
 * Creator work and removes both.
 * @param ctx - Owning plugin context.
 */
export declare function apply(ctx: Context): Promise<void>;
//# sourceMappingURL=index.d.ts.map