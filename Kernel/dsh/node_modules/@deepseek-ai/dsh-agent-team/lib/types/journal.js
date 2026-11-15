/** Serialized Team transactions over the exact live Lead Session log. */
import { TeamId } from "./types.js";
import { TeamError } from "./error.js";
/** Owns per-Lead transaction order and committed Team event publication. */
export class TeamJournal {
    ctx;
    teamProjectionHandle;
    tails = new Map();
    /**
     * @param ctx - Team service context with the injected Session service.
     * @param teamProjectionHandle - Team state registration owned by the Team service.
     */
    constructor(ctx, teamProjectionHandle) {
        this.ctx = ctx;
        this.teamProjectionHandle = teamProjectionHandle;
    }
    /**
     * Read authoritative Team state for one exact live Lead.
     * @param root - exact live Team Lead.
     * @returns current projected state selected by the Lead Team id.
     * @throws when the durable Team projection has failed.
     */
    state(root) {
        const projection = this.teamProjectionHandle.stateOf(root.session);
        if (projection.failure !== undefined)
            throw new Error(projection.failure);
        return projection;
    }
    /**
     * Serialize one Lead's asynchronous mutation operation.
     * @param rootId - Lead Session identity selecting the transaction queue.
     * @param operation - complete read-check-append operation.
     * @returns the operation result.
     */
    async transact(rootId, operation) {
        const prior = this.tails.get(rootId) ?? Promise.resolve();
        const run = prior.then(operation, operation);
        const tail = run.then(() => undefined, () => undefined);
        this.tails.set(rootId, tail);
        try {
            return await run;
        }
        finally {
            if (this.tails.get(rootId) === tail)
                this.tails.delete(rootId);
        }
    }
    /**
     * Append and flush one task revision.
     * @param root - live tree root whose Session stores the task.
     * @param task - complete committed task revision.
     * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED after append when flush fails or has no listener; read the task before retrying.
     */
    async appendAndFlush(root, task) {
        root.session.append('team/task', { version: 2, teamId: TeamId(root.id), task });
        try {
            const participated = await this.ctx.sessions.flush(root.session);
            if (!participated)
                throw new Error('No session durability listener participated');
        }
        catch (error) {
            throw new TeamError(`task "${task.id}" revision ${task.revision} is visible, but persistence was not confirmed; read the task before retrying`, 'TEAM_TASK_PERSISTENCE_UNCONFIRMED', { cause: error });
        }
    }
}
//# sourceMappingURL=journal.js.map