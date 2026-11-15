/** Shared task boards owned by subagent tree roots. */
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { Agent } from '@deepseek-ai/dsh-agent';
import { TeamTaskId } from './types.ts';
import type { Config, CreateTeamTaskRequest, TeamTaskView, UpdateTeamTaskRequest } from './types.ts';
export type * from './types.ts';
export { TeamId, TeamMessageId, TeamTaskId } from './types.ts';
export { TeamError } from './error.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        agentTeams: TeamService;
    }
}
/** Agent Teams service backed by the exact live Lead Session log. */
export declare class TeamService extends TypertRemoteService {
    static inject: string[];
    static Config: z<Config>;
    private projection;
    private readonly maxTasks;
    private readonly defaultEnabled;
    private journal;
    private tasks;
    constructor(ctx: Context, config?: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Read the Team selection shared by the caller's subagent tree.
     * @param caller - exact live root or descendant.
     * @returns whether Team tools and guidance are enabled.
     */
    isEnabled(caller: Agent): boolean;
    /**
     * Save a root Session's Team selection without stopping children or deleting tasks.
     * The selection takes effect before awaiting persistence. Tools resolve availability at
     * step preparation; prompt providers read the selection when evaluated. Task mutations
     * check the current mode when their queued operation starts; admitted operations finish.
     * @param agent - exact live root whose tree receives the selection.
     * @param enabled - selected Team mode.
     * @param signal - cancellation checked before changing the selection.
     * @returns fulfillment after the selection flushes.
     * @throws when called by a descendant or persistence fails. An appended selection remains active;
     * retrying the same selection flushes without a duplicate event.
     */
    setEnabled(agent: Agent, enabled: boolean, signal?: AbortSignal): Promise<void>;
    /**
     * Create one unowned pending task in the Team Lead log.
     * @param caller - exact live Team member creating the task.
     * @param request - task text, blockers, and advisory write scopes.
     * @param signal - cancellation before the task event is appended; omitted means no cancellation.
     * @returns the revision-one task view.
     * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED when the task is visible but durability is unknown; read it before retrying.
     */
    createTask(caller: Agent, request: CreateTeamTaskRequest, signal?: AbortSignal): Promise<TeamTaskView>;
    /**
     * Return one task, including a deleted tombstone.
     * @param caller - exact live Team member reading the task.
     * @param id - Team-local task identity.
     * @returns the latest task value and derived readiness diagnostics.
     */
    getTask(caller: Agent, id: TeamTaskId): TeamTaskView;
    /**
     * List current non-deleted tasks in numeric creation order.
     * @param caller - exact live Team member reading the board.
     * @returns detached current task views.
     */
    listTasks(caller: Agent): TeamTaskView[];
    /**
     * Compare-and-set one authorized task transition.
     * @param caller - exact live Team member authorizing the mutation.
     * @param request - task identity, expected revision, action, and action fields.
     * @param signal - cancellation before the task event is appended; omitted means no cancellation.
     * @returns the committed next task revision.
     * @throws TEAM_TASK_PERSISTENCE_UNCONFIRMED when the task is visible but durability is unknown; read it before retrying.
     */
    updateTask(caller: Agent, request: UpdateTeamTaskRequest, signal?: AbortSignal): Promise<TeamTaskView>;
}
export default TeamService;
//# sourceMappingURL=index.d.ts.map