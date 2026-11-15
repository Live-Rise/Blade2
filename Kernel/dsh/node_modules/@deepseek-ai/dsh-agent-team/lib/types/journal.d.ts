/** Serialized Team transactions over the exact live Lead Session log. */
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { ProjectionHandle } from '@deepseek-ai/dsh-session-projection';
import type { Context } from '@deepseek-ai/cordis';
import type { SessionId } from '@deepseek-ai/dsh-session';
import type { TeamProjectionState, TeamState } from './projection.ts';
import type { TeamTaskSnapshot } from './types.ts';
/** Owns per-Lead transaction order and committed Team event publication. */
export declare class TeamJournal {
    private readonly ctx;
    private readonly teamProjectionHandle;
    private readonly tails;
    /**
     * @param ctx - Team service context with the injected Session service.
     * @param teamProjectionHandle - Team state registration owned by the Team service.
     */
    constructor(ctx: Context, teamProjectionHandle: ProjectionHandle<TeamProjectionState>);
    /**
     * Read authoritative Team state for one exact live Lead.
     * @param root - exact live Team Lead.
     * @returns current projected state selected by the Lead Team id.
     * @throws when the durable Team projection has failed.
     */
    state(root: Agent): TeamState;
    /**
     * Serialize one Lead's asynchronous mutation operation.
     * @param rootId - Lead Session identity selecting the transaction queue.
     * @param operation - complete read-check-append operation.
     * @returns the operation result.
     */
    transact<T>(rootId: SessionId, operation: () => Promise<T>): Promise<T>;
    /**
     * Append and flush one task revision.
     * @param root - live tree root whose Session stores the task.
     * @param task - complete committed task revision.
     * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED after append when flush fails or has no listener; read the task before retrying.
     */
    appendAndFlush(root: Agent, task: TeamTaskSnapshot): Promise<void>;
}
//# sourceMappingURL=journal.d.ts.map