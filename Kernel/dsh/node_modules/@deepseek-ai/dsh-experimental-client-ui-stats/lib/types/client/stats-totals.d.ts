/** Whole-session totals from the host projections that survive client paging. */
import type { SessionStatsProjection } from '@deepseek-ai/dsh-session-stats/types';
import type { TokenUsageProjection } from '@deepseek-ai/dsh-token-meter/client';
import type { StatsTotals } from './stats-aggregate.ts';
/**
 * Map the whole-session stats and token-usage projections onto the totals row.
 * @param stats - whole-log turn/step and wall-time projection.
 * @param usage - whole-log prompt and output token projection.
 * @returns totals over the complete session, or null while either projection is absent.
 */
export declare function totalsFromProjections(stats: SessionStatsProjection | undefined, usage: TokenUsageProjection | undefined): StatsTotals | null;
//# sourceMappingURL=stats-totals.d.ts.map