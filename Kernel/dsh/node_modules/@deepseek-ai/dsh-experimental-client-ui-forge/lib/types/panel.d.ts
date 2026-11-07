/** Live panel state of one followed Session, shared by all of its followers. */
import type { Context } from '@deepseek-ai/cordis';
import type { Session, SessionEvent } from '@deepseek-ai/dsh-session';
import type { ProjectionHandle } from '@deepseek-ai/dsh-session-projection';
import type { GitConfig } from './git.ts';
import type { DiffConfig } from './local-diff.ts';
import type { ForgeMissingWorktree, ForgePanelState, ForgeWorktreeState } from './types.ts';
import type { WorktreeCreations } from './worktree.ts';
/** Git and diff limits plus the sign-in probe deadline. */
export interface PanelConfig extends DiffConfig {
    /** Deadline in milliseconds for one provider sign-in probe. */
    readonly readinessTimeoutMs: number;
}
/**
 * Panel state of one Session. Construction starts the first checkout
 * inspection and watches the Session's change requests. Inspections run one at
 * a time; a request that arrives during one runs once more after it, and a
 * result for a directory the Session has since left is dropped. A sign-in
 * probe runs when an inspection recognizes a new code host and on `refresh`;
 * a probe result for a host the panel no longer shows is dropped. Each change
 * request keeps the latest snapshot a poll returned while later polls fail.
 * When an inspection finds an upstream branch on a remote a provider
 * recognizes, the open change requests of that branch are bound as discovered,
 * once per remote and branch until a refresh. The
 * worktree facts follow the `forgeWorktree` projection, the live creation, and the last failed one;
 * each inspection also decides whether the Session's sandbox can write the
 * checkout and its shared Git directory, which a new worktree needs, whether
 * the worktree the Session created and works in is gone, and how many lines
 * the `branch` scope changes; a measurement that fails leaves that null.
 */
export declare class SessionPanel {
    private readonly ctx;
    private readonly config;
    readonly session: Session;
    private readonly worktreeStates;
    private readonly creations;
    private readonly onIdle;
    private current;
    private readonly followers;
    private readonly lifetime;
    private readonly tasks;
    private readonly stopWatching;
    private readonly snapshots;
    private blocker;
    private missing;
    private inspection;
    private rerun;
    /** Remote URL and branch of the upstream that the last branch lookup covered, until a refresh. */
    private discovered;
    /**
     * @param ctx - service context that provides the forge, Session, directory, and process services.
     * @param config - Git limits and the sign-in probe deadline.
     * @param session - Session the panel describes.
     * @param worktreeStates - reads the `forgeWorktree` projection of the Session.
     * @param creations - reads the running creation and restoration, the last failed creation, and what a message of the
     * Session waits for.
     * @param onIdle - called when the last follower leaves.
     */
    constructor(ctx: Context, config: PanelConfig, session: Session, worktreeStates: ProjectionHandle<ForgeWorktreeState>, creations: Pick<WorktreeCreations, 'of' | 'restorationOf' | 'failureOf' | 'waitingOf'>, onIdle: () => void);
    /**
     * Yield the current state, then each later state, until `signal` aborts or the panel stops.
     * @param signal - ends this follower.
     * @returns the states in publication order; intermediate states may be skipped.
     */
    follow(signal: AbortSignal): AsyncGenerator<ForgePanelState>;
    /**
     * React to one committed event of the Session: a directory change moves the
     * panel and reinspects; a tool result reinspects, because the tool may have
     * changed the checkout; a sandbox mode change reinspects, because it decides
     * whether a worktree can be created; the first turn, the first step, and
     * worktree records update the worktree facts.
     * @param event - committed Session event.
     */
    observe(event: SessionEvent): void;
    /** Republish the worktree facts when they changed, such as after a creation moved or a worktree service appeared or left. */
    updateWorktree(): void;
    /** Forget the missing worktree once it was checked out again; the next publication carries it, and a reinspection confirms it. */
    restored(): void;
    /** Reinspect the checkout, such as after its directory was created again. */
    reinspect(): void;
    /**
     * Check again whether the worktree the Session works in is gone, publishing a change.
     * @param signal - cancellation of the check.
     * @returns the unavailable worktree, or null for a valid checkout or a Session working elsewhere.
     * @throws when the filesystem or Git cannot answer.
     */
    checkMissing(signal: AbortSignal): Promise<ForgeMissingWorktree | null>;
    /**
     * Reinspect the checkout, probe sign-in again, and poll every bound change request at once.
     * @returns settles once the inspection and the sign-in probe have ended; each poll publishes its result when it finishes.
     */
    refresh(): Promise<void>;
    /**
     * Stop inspections, probes, and the change request watch, end every follower, and wait for pending work.
     * @returns settles once pending inspections and probes have ended.
     */
    dispose(): Promise<void>;
    private entries;
    private worktree;
    private publish;
    /** Track `task` for disposal; the returned promise resolves once it settles, whatever the outcome. */
    private run;
    /**
     * Inspect the checkout, one inspection at a time; calls during an
     * inspection share one trailing rerun.
     * @returns resolves once an inspection that started after the call has ended.
     */
    private inspect;
    private inspectOnce;
    /** The upstream branch with the first URL of its remote that a provider recognizes. */
    private upstreamOf;
    /** Bind the open change requests of the upstream branch, once per remote and branch until a refresh. */
    private discover;
    /** Decide whether the Session's sandbox lets the worktree service write the checkout and its shared Git directory. */
    private blockerOf;
    /** Measure the `branch` scope; a failed measurement only hides the local changes card. */
    private localOf;
    private recognize;
    /** Probe sign-in on the repository's host; resolves once the probe has ended. */
    private probe;
}
/**
 * Decide whether a Session works inside the worktree it created while that
 * worktree's directory is gone, and whether its branch still exists.
 * @param ctx - context that provides `fs` and the process services.
 * @param config - Git limits.
 * @param session - Session to check.
 * @param state - recorded worktree and restoration completion.
 * @param directory - the Session's current directory.
 * @param signal - cancellation of the check.
 * @returns the unavailable worktree, or null for a valid checkout or a Session working elsewhere.
 * @throws when the filesystem or Git cannot answer.
 */
export declare function missingWorktree(ctx: Context, config: GitConfig, session: Session, state: ForgeWorktreeState, directory: string, signal: AbortSignal): Promise<ForgeMissingWorktree | null>;
//# sourceMappingURL=panel.d.ts.map