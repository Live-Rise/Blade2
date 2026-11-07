/** View target builder turning published nodes into turn rows. */
import type { Context } from '@deepseek-ai/cordis';
import type { ConversationViewDefinition } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { StatsSnapshot, StatsViewNode } from './stats-contract.ts';
/** Stable snapshot used before a Session assembled any performance node. */
export declare const EMPTY_STATS_SNAPSHOT: StatsSnapshot;
/** The `stats` target every Definition publishes into. */
export declare const statsViewDefinition: ConversationViewDefinition<StatsViewNode, StatsSnapshot>;
/**
 * Register the target builder. The registration rides the caller's fiber
 * effect, so unloading the plugin drops the target with it.
 * @param ctx - client root context.
 */
export declare function registerStatsConversationView(ctx: Context): void;
//# sourceMappingURL=stats-snapshot-builder.d.ts.map