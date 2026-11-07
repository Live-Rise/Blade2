import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { SessionProjectionSummaryValues } from '@deepseek-ai/dsh-session-projection/types';
import type { SessionSummary } from '../../types.ts';
/** Host list summary enriched with the latest Session Controller title projection. */
export interface TitledSessionSummary extends SessionSummary {
    title?: string;
    /**
     * The row's summary projection values, never the full projection set. A
     * connected Session's client store may hold more values from its
     * control/follow baseline; those are outside the list contract and must not
     * be read here.
     */
    projectionValues?: Readonly<Partial<SessionProjectionSummaryValues>>;
}
/** One flattened session-list row with lineage depth. */
export interface SessionListEntry {
    sessionId: SessionId;
    title?: string;
    updatedAt: number;
    running: boolean;
    /**
     * New Session presentation and reuse eligibility, reconciled with
     * `sessionListMetadata`; lists hide blank sessions (filtering stays with the consumer).
     */
    blank: boolean;
    parentSessionId?: SessionId;
    /** Coarse durable origin for navigation filtering; not a continuation capability. */
    origin?: 'subagent';
    /** Format classification supplied by the Host list. */
    formatStatus?: SessionSummary['formatStatus'];
    cwd?: string;
    /**
     * The row's summary projection values, never the full projection set; see
     * `TitledSessionSummary.projectionValues`.
     */
    projectionValues?: Readonly<Partial<SessionProjectionSummaryValues>>;
    /** Lineage indent depth: root = 0; the UI just multiplies by the indent width. */
    depth: number;
}
/**
 * Summaries -> flat list with lineage indentation. Root and sibling order
 * follows the established input order; this projection never re-sorts a
 * hydrated list from mutable timestamps.
 * @param summaries - the host's session.list items.
 * @returns display rows in render order.
 */
export declare function flattenLineage(summaries: readonly TitledSessionSummary[]): SessionListEntry[];
//# sourceMappingURL=lineage.d.ts.map