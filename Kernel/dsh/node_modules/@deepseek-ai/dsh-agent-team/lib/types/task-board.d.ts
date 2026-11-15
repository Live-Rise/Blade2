/** Shared Team task DAG commands and runtime-enriched views. */
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { Context } from '@deepseek-ai/cordis';
import type { TeamJournal } from './journal.ts';
import { TeamTaskId } from './types.ts';
import type { CreateTeamTaskRequest, TeamTaskView, UpdateTeamTaskRequest } from './types.ts';
/** Owns Team task limits, authorization, transitions, and derived views. */
export declare class TeamTaskBoard {
    private readonly ctx;
    private readonly journal;
    private readonly maxTasks;
    /**
     * @param ctx - subagent discovery for task assignees.
     * @param journal - authoritative Lead-log transaction owner.
     * @param maxTasks - maximum non-deleted tasks retained by one Team.
     */
    constructor(ctx: Context, journal: TeamJournal, maxTasks: number);
    private state;
    /**
     * Create one unowned pending task in the Team Lead log.
     * @param caller - exact live Agent creating the task.
     * @param root - live subagent tree root owning the board.
     * @param request - task text, blockers, and advisory write scopes.
     * @param signal - cancellation before append; omitted means no cancellation.
     * @returns the revision-one task view.
     */
    create(caller: Agent, root: Agent, request: CreateTeamTaskRequest, signal?: AbortSignal): Promise<TeamTaskView>;
    /**
     * Return one task, including a deleted tombstone.
     * @param root - live subagent tree root owning the board.
     * @param id - Team-local task identity.
     * @returns the latest task value and derived readiness diagnostics.
     */
    get(root: Agent, id: TeamTaskId): TeamTaskView;
    /**
     * List current non-deleted tasks in numeric creation order.
     * @param root - live subagent tree root owning the board.
     * @returns detached current task views.
     */
    list(root: Agent): TeamTaskView[];
    /**
     * Compare-and-set one authorized task transition.
     * @param caller - exact live Team member authorizing the mutation.
     * @param root - live subagent tree root owning the board.
     * @param request - task identity, expected revision, action, and action fields.
     * @param signal - cancellation before append; omitted means no cancellation.
     * @returns the committed next task revision.
     */
    update(caller: Agent, root: Agent, request: UpdateTeamTaskRequest, signal?: AbortSignal): Promise<TeamTaskView>;
    /** Resolve a task owner through the same durable tree used by Agent discovery. */
    private resolveAssignee;
    /** Validate and de-duplicate dependency ids against the current task graph. */
    private dependencies;
    /** Normalize and de-duplicate task write scopes. */
    private writeScopes;
    /** Map shared task-graph validation onto stable command error codes. */
    private assertTaskGraph;
    /** Remove both optional owner fields under exactOptionalPropertyTypes. */
    private withoutOwner;
}
//# sourceMappingURL=task-board.d.ts.map