/** Team state projected incrementally from committed Session events, with a durable-only client view. */
import { z } from 'zod';
import type { SessionEvent, SessionId, SessionLogOffset } from '@deepseek-ai/dsh-session';
import type { TeamId, TeamProjection, TeamTaskSnapshot } from './types.ts';
/**
 * Task board and saved mode selected by durable Team identity. Mode changes
 * preserve the task collection reference.
 */
export interface TeamState {
    /** Saved selection; absent until an explicit selection or historical task. */
    readonly enabled?: boolean;
    readonly id: TeamId;
    readonly tasks: readonly TeamTaskSnapshot[];
    readonly nextTaskNumber: number;
}
/**
 * Construct empty state for one Team identity.
 * @param rootId - root Session identity.
 * @param inheritedEventCount - inherited prefix excluded from the new root's Team mode; defaults to zero.
 * @returns empty Team state.
 */
export declare function emptyTeamState(rootId: SessionId, inheritedEventCount?: number): TeamProjectionState;
/** Checkpoint-safe state for the Team owned by the projected Session. */
export interface TeamProjectionState extends TeamState {
    readonly inheritedEventCount: number;
    readonly failure?: string;
}
declare function applyProjectionEvent(state: TeamProjectionState, event: SessionEvent): TeamProjectionState;
/**
 * Durable client view of one Team state, reused while tasks and mode remain unchanged.
 * A task-board failure retains the last valid tasks; mode selections still apply.
 * @param state - current Team state.
 * @returns the saved selection and non-deleted task board, plus any projection failure.
 */
export declare function teamProjectionView(state: TeamProjectionState): TeamProjection;
/**
 * Team projection selected by the projected Session identity; the wire view
 * carries the selection and task board. Current subagents are
 * supplied separately by the existing `subagentCatalog` projection.
 * Version-7 checkpoints without owner names remain usable without backfilling.
 */
export declare const teamProjectionDefinition: {
    key: "agentTeam";
    stateVersion: number;
    stateSchema: z.ZodType<TeamProjectionState, unknown, z.core.$ZodTypeInternals<TeamProjectionState, unknown>>;
    init: (header: import("@deepseek-ai/dsh-session").SessionHeader, inheritedEventCount: SessionLogOffset) => TeamProjectionState;
    apply: typeof applyProjectionEvent;
    wire: {
        viewSchema: z.ZodType<TeamProjection, unknown, z.core.$ZodTypeInternals<TeamProjection, unknown>>;
        view: typeof teamProjectionView;
    };
};
export {};
//# sourceMappingURL=projection.d.ts.map