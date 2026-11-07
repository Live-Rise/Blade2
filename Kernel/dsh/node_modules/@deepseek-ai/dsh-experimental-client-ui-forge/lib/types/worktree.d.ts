/** The worktree a Session requests before it starts: its record, its projection, and its creation. */
import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { CreatedWorktree, CreateWorktreePhase } from '@deepseek-ai/dsh-experimental-worktree';
import type { Session, SessionEvent } from '@deepseek-ai/dsh-session';
import type { ProjectionHandle } from '@deepseek-ai/dsh-session-projection';
import { z } from 'zod';
import type { ForgeMissingWorktree, ForgeSessionWorktree, ForgeWorktreeCreation, ForgeWorktreeFailure, ForgeWorktreeRestoration, ForgeWorktreeState, ForgeWorktreeWaiting } from './types.ts';
/** Plugin record type of worktree requests, withdrawals, and creations. */
export declare const WORKTREE_RECORD = "plugin:client-ui-forge/worktree";
/**
 * The `forgeWorktree` unit. `request` and `cancel` records change the request
 * until the first `turn/start` or creation; the first `created` record of an
 * open request applies until the first `step/start`. Records outside those
 * periods leave the state unchanged. The unit also follows the Session's
 * working directory, so clients, including the Session summary, see the
 * created worktree only while that directory is the worktree or lies below it.
 * Restoration records retain incomplete checkout recovery across reloads.
 */
export declare const forgeWorktreeProjection: {
    key: "forgeWorktree";
    stateVersion: number;
    stateSchema: z.ZodType<ForgeWorktreeState, unknown, z.core.$ZodTypeInternals<ForgeWorktreeState, unknown>>;
    init: (header: import("@deepseek-ai/dsh-session").SessionHeader) => {
        cwd: string | null;
        request: import("./types.ts").ForgeWorktreeRequest | null;
        created: ForgeSessionWorktree | null;
        restoring: boolean;
        locked: boolean;
        stepped: boolean;
    };
    apply: typeof apply;
    wire: {
        viewSchema: z.ZodNullable<z.ZodType<ForgeSessionWorktree, unknown, z.core.$ZodTypeInternals<ForgeSessionWorktree, unknown>>>;
        view: (state: NoInfer<ForgeWorktreeState>) => ForgeSessionWorktree | null;
    };
};
/**
 * Whether `path` is `root` or lies below it.
 * @param root - absolute directory.
 * @param path - absolute path.
 * @returns whether `root` contains `path`.
 */
export declare function within(root: string, path: string): boolean;
/**
 * The text of a failure: an Error's message, or else the thrown value as a string.
 * @param error - what was thrown.
 * @returns the text.
 */
export declare function messageOf(error: unknown): string;
declare function apply(state: ForgeWorktreeState, event: SessionEvent): ForgeWorktreeState;
/** A failed attempt to create a requested worktree; `cause` holds the worktree service's error. */
export declare class CreationFailure extends Error {
    readonly branch: string;
    readonly phase: CreateWorktreePhase;
    readonly reason: string;
    /**
     * @param branch - branch the attempt created.
     * @param phase - step that failed.
     * @param reason - the worktree service's failure text.
     * @param cause - the worktree service's error.
     */
    constructor(branch: string, phase: CreateWorktreePhase, reason: string, cause: unknown);
}
/** Work on a Session's checkout that runs as maintenance of its Agent. */
export type HeldWork = (agent: Agent, signal: AbortSignal) => Promise<void>;
/**
 * Creates requested worktrees, one attempt with a new branch name per call,
 * and holds the turn of a message while the Session's worktree is not ready.
 * A hold is maintenance of the Session's Agent that starts while the message
 * enters the inbox, before the delivery wakes the Agent, so the Agent starts
 * the message's turn once the hold ends. A creation hold makes an attempt
 * and, after a failed or stopped one, waits for another attempt or for the
 * request to be withdrawn; a restoration hold checks the worktree the Session
 * works in and, when it is gone, waits until it is restored or left. The
 * Agent's cancellation and the panel's stop end a hold. The first step
 * proposal of a Session whose request is still open also creates the
 * worktree, which covers a message no hold took. The worktree service enters
 * the created checkout and queues its directory notice; at a step proposal
 * the notice joins the proposed messages so the first request already names
 * the new directory, a proposal without messages leaves it queued, and a
 * failure ends the turn with the request still open. A failed attempt may
 * leave its branch or checkout behind.
 */
export declare class WorktreeCreations {
    private readonly ctx;
    private readonly states;
    private readonly prefix;
    private readonly changed;
    private readonly live;
    private readonly restorations;
    private readonly failures;
    private readonly holds;
    private readonly lifetime;
    private readonly pending;
    /** Stop admission, cancel operations, and wait for their final callbacks. */
    readonly dispose: () => Promise<void>;
    /**
     * @param ctx - context that optionally provides `worktrees`.
     * @param states - reads the `forgeWorktree` projection of a Session.
     * @param prefix - branch name prefix that `generatedWorktreeName` combines with the Session id and random digits.
     * @param changed - called after the creation or restoration of a Session starts, changes phase, or ends, after its
     * failure changes, and after a wait starts or ends.
     */
    constructor(ctx: Context, states: ProjectionHandle<ForgeWorktreeState>, prefix: string, changed: (session: Session) => void);
    /**
     * Read the running creation of a Session.
     * @param session - Session to read.
     * @returns the running creation, or null.
     */
    of(session: Session): ForgeWorktreeCreation | null;
    /**
     * Read the running restoration of a Session.
     * @param session - Session to read.
     * @returns the running restoration, or null.
     */
    restorationOf(session: Session): ForgeWorktreeRestoration | null;
    /**
     * Read the failed attempt that a waiting message saw last.
     * @param session - Session to read.
     * @returns the failure, or null.
     */
    failureOf(session: Session): ForgeWorktreeFailure | null;
    /**
     * Read what a message of a Session waits for before its turn starts.
     * @param session - Session to read.
     * @returns the creation or restoration the message waits for, or null while none waits.
     */
    waitingOf(session: Session): ForgeWorktreeWaiting | null;
    /**
     * Forget the failure of a Session whose request changes or is withdrawn; the record of that change republishes the panel.
     * @param session - Session whose failure no longer applies.
     */
    forget(session: Session): void;
    /**
     * Hold the turn of a message entering the inbox of a Session whose
     * requested worktree does not exist yet, and create the worktree. Call it
     * while the message enters, before the delivery wakes the Agent. A Session
     * already held, or an Agent with other work, gets no new hold.
     * @param agent - Agent of the Session.
     */
    holdCreation(agent: Agent): void;
    /**
     * Hold the turn of a message entering the inbox of a Session that works in
     * its created worktree while `check` decides whether that worktree is gone,
     * and until it is restored or left when it is. Call it while the message
     * enters, before the delivery wakes the Agent. A Session already held, or an
     * Agent with other work, gets no new hold.
     * @param agent - Agent of the Session.
     * @param check - answers whether the worktree is gone; a rejection lets the message go on.
     */
    holdRestoration(agent: Agent, check: (signal: AbortSignal) => Promise<ForgeMissingWorktree | null>): void;
    /**
     * Make another attempt for the message that waits after a failed or stopped one.
     * @param session - Session whose message waits.
     * @returns whether a waiting message without a running attempt took the decision.
     */
    retry(session: Session): boolean;
    /**
     * Read the request again after it changed; a withdrawn request lets the waiting message go on.
     * @param session - Session whose request changed.
     */
    recheck(session: Session): void;
    /**
     * Run work on the removed worktree a message waits for, inside the wait; its success lets the message go on.
     * @param session - Session whose message waits.
     * @param work - restores or leaves the worktree.
     * @returns the work's outcome, or undefined when no message waits for the removed worktree.
     */
    within(session: Session, work: HeldWork): Promise<void> | undefined;
    /**
     * Stop the running attempt a message waits for; the message keeps waiting
     * with the request still open. Without such an attempt the Session is unchanged.
     * @param session - Session whose attempt stops.
     */
    stop(session: Session): void;
    /**
     * Check the removed worktree of a Session out again through the worktree
     * service; the running restoration is readable through
     * {@link WorktreeCreations.restorationOf} until it ends.
     * @param agent - Agent of the Session.
     * @param worktrees - the worktree service.
     * @param created - the worktree the Session created.
     * @param signal - cancellation of the restoration.
     * @param restored - runs once the checkout is back, before the restoration ends, so both changes publish together.
     * @param available - checks whether the recorded checkout is already usable, within this operation's lifetime.
     * @throws the worktree service's error when the restoration fails; the cancellation reason once `signal` aborts.
     */
    restore(agent: Agent, worktrees: Context['worktrees'], created: ForgeSessionWorktree, signal: AbortSignal, restored: () => void, available: (signal: AbortSignal) => Promise<boolean>): Promise<void>;
    private restoreCheckout;
    /**
     * Create a Session's requested worktree with a new branch name, enter it, and
     * record it on the Session. The running creation is readable through
     * {@link WorktreeCreations.of} until the record exists or the attempt fails.
     * @param agent - Agent of the Session.
     * @param from - base branch, or null for HEAD of the current checkout.
     * @param signal - cancellation of the attempt.
     * @param held - whether a message waits for the creation before its turn starts.
     * @returns the created worktree.
     * @throws {@link CreationFailure} when the attempt fails; the cancellation reason once `signal` aborts.
     */
    create(agent: Agent, from: string | null, signal: AbortSignal, held: boolean): Promise<CreatedWorktree>;
    private createCheckout;
    /**
     * Run a hold as maintenance of the Agent, which latches the delivery's wake until the hold ends.
     * @param agent - Agent of the Session.
     * @param hold - the hold's state.
     * @param body - waits for the worktree; settles once the message may go on.
     */
    private hold;
    private run;
    private join;
    /**
     * Make attempts until one creates the worktree or the request no longer asks
     * for one, waiting for a decision after each failed or stopped attempt.
     */
    private runCreation;
    /** Check the worktree the Session works in and, when it is gone, run the work that restores or leaves it until one succeeds. */
    private runRestoration;
    /** Wait for the next decision on a hold; rejects with the abort reason once `signal` aborts. */
    private wait;
    private preStep;
}
export {};
//# sourceMappingURL=worktree.d.ts.map