/** Creates a new Git branch and checkout, then changes the calling Session's working directory; restores a removed checkout. */
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { CreatedWorktree, CreateWorktreePhase, CreateWorktreeRequest, RestoredWorktree, RestoreWorktreeRequest } from './types.ts';
export type { CreatedWorktree, CreateWorktreePhase, CreateWorktreeRequest, RestoredWorktree, RestoreWorktreeRequest } from './types.ts';
export { generatedWorktreeName } from './name.ts';
/** Worktree location, generated names, executables, and subprocess resource limits. */
export interface Config {
    /** Relative checkout pool inside the source repository. */
    directory?: string;
    /** Prefix for automatically generated branch and checkout names. */
    namePrefix?: string;
    /** Executable name or absolute execution-world path of a Git release that honors `GIT_NO_LAZY_FETCH`. */
    gitCommand?: string;
    /** Node executable used for sandboxed directory allocation. */
    nodeCommand?: string;
    /** Deadline in milliseconds for one command. */
    timeoutMs?: number;
    /** Subprocess termination grace in milliseconds. */
    graceMs?: number;
    /** Maximum captured bytes per subprocess output stream. */
    maxOutputBytes?: number;
}
declare module '@deepseek-ai/cordis' {
    interface Context {
        worktrees: WorktreeService;
    }
}
/** Creates retained Git worktrees under the mounted filesystem and sandbox providers. */
export declare class WorktreeService extends Service {
    static inject: string[];
    static Config: z<Config>;
    private readonly config;
    private readonly lifetime;
    private readonly pending;
    constructor(ctx: Context, config: Config);
    /**
     * Create a fresh branch and checkout at a pinned local revision, then enter it.
     * Existing branches or checkout paths fail. Uncommitted files stay in the source checkout.
     * Checkout disables configured clean, smudge, and process filters without changing Git config.
     * Repository-local replacement refs still apply; baseCommit reports the resolved object name.
     * The new checkout becomes current only after Git setup succeeds. Failures may retain newly
     * allocated Git/filesystem artifacts; no branch or checkout is removed automatically.
     * @param agent - caller whose current directory selects the source repository and file policy.
     * @param request - optional new name and local revision; defaults are generated name and HEAD.
     * @param signal - cancellation of lookup, creation, and working-directory publication.
     * @param onPhase - receives each stage before it begins; it must not throw.
     * @returns canonical checkout path, branch name, pinned commit, and source repository root.
     */
    create(agent: Agent, request?: CreateWorktreeRequest, signal?: AbortSignal, onPhase?: (phase: CreateWorktreePhase) => void): Promise<CreatedWorktree>;
    /**
     * Check out a removed worktree again at its recorded directory: its branch
     * when the branch still exists, otherwise a new branch of the same name at
     * `baseCommit`. Only the missing target's unlocked registration is removed, and an
     * existing directory fails. Changes that were never committed are not
     * recovered. Checkout disables configured clean, smudge, and process filters,
     * as creation does. The calling Session's working directory does not change.
     * Failures may retain newly allocated Git/filesystem artifacts.
     * @param agent - caller whose sandbox policy confines the commands.
     * @param request - recorded directory, branch, base commit, and source repository root.
     * @param signal - cancellation of the restoration.
     * @param onPhase - receives each stage before it begins; it must not throw.
     * @returns the directory and whether the branch was created again.
     */
    restore(agent: Agent, request: RestoreWorktreeRequest, signal?: AbortSignal, onPhase?: (phase: CreateWorktreePhase) => void): Promise<RestoredWorktree>;
    private resolve;
    private createCheckout;
    private restoreCheckout;
    /** Confine Git commands to the caller's sandbox policy, with hooks and the filesystem monitor disabled. */
    private gitRunner;
    /** Allocate the checkout directory exclusively inside the pool, creating a self-ignoring pool when it is new. */
    private prepareDirectory;
}
export default WorktreeService;
//# sourceMappingURL=index.d.ts.map