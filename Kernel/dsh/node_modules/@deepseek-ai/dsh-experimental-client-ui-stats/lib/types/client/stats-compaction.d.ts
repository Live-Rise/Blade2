/** Compaction transaction state and its presentation as a synthetic step row. */
import type { ConversationNodeDefinition } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { StatsCompactionMetrics, StatsCompactionState } from './stats-contract.ts';
/** Event shape a Conversation Definition receives. */
type MatchEvent = Parameters<ConversationNodeDefinition['match']>[0];
/**
 * Compaction id an event names.
 * @param event - event seen by the Definition match.
 * @returns the nonempty compaction id, or null for events outside a transaction.
 */
export declare function compactionEventId(event: MatchEvent): string | null;
/**
 * Start state for one `compaction/start`.
 * @param event - the start event.
 * @returns state that later summary and end events refine.
 */
export declare function newCompactionState(event: MatchEvent): StatsCompactionState;
/**
 * Fold the summary or end of a compaction into its state.
 * @param state - state opened by compaction/start.
 * @param event - a matching compaction event.
 * @returns state after the event, or the same state for unrelated events.
 */
export declare function foldCompaction(state: StatsCompactionState, event: MatchEvent): StatsCompactionState;
/**
 * View metrics of one compaction transaction.
 * @param state - folded compaction state.
 * @returns the row values; unavailable measurements stay null.
 */
export declare function compactionMetrics(state: StatsCompactionState): StatsCompactionMetrics;
export {};
//# sourceMappingURL=stats-compaction.d.ts.map