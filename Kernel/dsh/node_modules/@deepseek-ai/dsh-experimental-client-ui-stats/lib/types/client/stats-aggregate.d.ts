/** Turn aggregation over the step metrics a window holds. */
import type { StatsStepMetrics, StatsTurnMetrics, StatsViewNode } from './stats-contract.ts';
/**
 * Aggregate one turn's steps into the row values the table shows.
 * @param turn - turn number.
 * @param steps - the turn's step metrics in step order.
 * @returns the turn row.
 */
export declare function aggregateTurn(turn: number, steps: readonly StatsStepMetrics[]): StatsTurnMetrics;
/**
 * Group published nodes into ordered turns, closing each turn with its markers.
 * Code-Mode contributions merge into the step that dispatched them, identified
 * by the root call id the node carries.
 * @param nodes - every node the target builder currently holds.
 * @returns turn rows ordered oldest first.
 */
export declare function buildTurns(nodes: Iterable<StatsViewNode>): StatsTurnMetrics[];
/** How a turn or window aggregates prompt sizes: final prompt or cumulative processing. */
export type ContextMode = 'latest' | 'cumulative';
/** Totals over the turns loaded in the current window. */
export interface StatsTotals {
    readonly turnCount: number;
    readonly stepCount: number;
    readonly context: number | null;
    readonly output: number | null;
    readonly cacheHit: number | null;
    readonly cacheRead: number | null;
    readonly cacheWrite: number | null;
    readonly cacheContext: number | null;
    readonly ttft: number | null;
    readonly ttftPending: number;
    readonly model: number | null;
    readonly modelPending: number;
    readonly tps: number | null;
    readonly tool: number | null;
    readonly toolPending: number;
    readonly toolWall: number | null;
    readonly wall: number | null;
}
/**
 * Context of one turn under the selected basis.
 * @param turn - aggregated turn.
 * @param mode - latest final prompt size, or the sum of its step prompt sizes.
 * @returns the context tokens, or null when no step measured one.
 */
export declare function turnContext(turn: StatsTurnMetrics, mode: ContextMode): number | null;
/**
 * Aggregate the loaded turns into one totals row.
 * @param turns - aggregated turns in display order.
 * @returns summed metrics; the context column is always the cumulative step context.
 */
export declare function aggregateTotals(turns: readonly StatsTurnMetrics[]): StatsTotals;
//# sourceMappingURL=stats-aggregate.d.ts.map