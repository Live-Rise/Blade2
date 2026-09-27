/**
 * Hero-chip controller: which preset the NEXT session gets.
 *
 * The new-session screen has no session, so a pick is staged rather than
 * applied. It reaches a session when one becomes current and is still blank —
 * whether the workspace connect created it or reused an existing blank one,
 * which is why staging cannot simply ride along on `sessions.create`.
 *
 * The stage is forgotten once applied. The next new session starts from the
 * Host-effective default again.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { SessionSummary } from '@deepseek-ai/dsh-api-session-controller/client';
import { type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { AgentPresetOption } from './settings-store.ts';
/** Hero-chip snapshot. */
export interface AgentPresetSeatState {
    /** Presets the deployment supplies; empty means the chip renders nothing. */
    options: readonly AgentPresetOption[];
    /** The staged choice, empty until the roster loads. */
    current: string;
    /** A rejected apply's message, cleared by the next attempt. */
    error: string | null;
    busy: boolean;
    /**
     * One-shot cue that the chip should introduce itself (the creator-draft
     * entry staged the pick from another screen, so the user never touched the
     * chip); the renderer clears it via `introduced()` once played.
     */
    introduce: boolean;
}
/** Mutable one-shot preset choice shared across Provider-bound seat controllers. */
export interface AgentPresetStage {
    /** Preset awaiting application; absence means no staged choice. */
    id: string | undefined;
    /** Whether the receiving chip should announce the applied choice once. */
    introduce: boolean;
}
/** Stages the next session's preset and applies it when one appears. */
export declare class AgentPresetSeatController {
    private readonly ctx;
    /** The session the hero is about to hand over to, when there is one. */
    private readonly currentSession;
    private readonly staged;
    /** Chip snapshot the renderer subscribes to. */
    readonly store: SnapshotStore<AgentPresetSeatState>;
    /**
     * The Host-effective default, so a consumed stage can fall back to it without
     * re-reading the roster.
     */
    private fallback;
    /** Only the newest roster read may publish after overlapping refreshes. */
    private loadGeneration;
    /** Completion of the active Host selection; Settings choices wait before staging. */
    private pendingSelection;
    constructor(ctx: ClientContext, 
    /** The session the hero is about to hand over to, when there is one. */
    currentSession: () => Pick<SessionSummary, 'id' | 'blank' | 'projectionValues'> | undefined, staged?: AgentPresetStage);
    private set;
    private clearStage;
    /** Developer tools are the single gate over preset selection. */
    private selectionAvailable;
    /**
     * Read the roster and open the chip on the Host-effective default.
    * @returns once the snapshot reflects the host.
    */
    load(): Promise<void>;
    /**
     * Stage one preset for the next session, applying it immediately when a
     * blank session is already current.
     *
     * The refusal is returned as well as stored, because the two readers need
     * different things from it: the chip's own label carries the standing state,
     * while the caller that made this pick is the one that has to say why the
     * label came back — and only it knows the pick was a person's, not the
     * applier catching up with a session that just became current.
     * @param id - the preset to stage.
     * @returns the refusal text, or undefined once the pick settled.
     */
    select(id: string): Promise<string | undefined>;
    /**
     * Stage a pick WITHOUT the immediate apply, for a flow that starts the
     * receiving session after the pick (the settings section's creator entry).
     * `select()`'s immediate apply would meet the still-current running session
     * and drop the stage as unservable; staging alone leaves it for the
     * list-change applier, which fires when the started session becomes
     * current.
     * @param id - the preset to stage.
     * @param introduce - true when the stage came from another screen and the
     * chip should announce itself on the session it lands on.
     */
    stage(id: string, introduce?: boolean): void;
    /**
     * Capture the exact blank Session a Settings action may bring along.
     * @returns its id, or undefined outside a blank Session.
     */
    blankSessionId(): SessionSummary['id'] | undefined;
    /**
     * Apply a Settings choice only if its captured Session is still current and
     * blank after any pending selection settles. The selection uses the existing stage/apply path.
     * @param expectedSessionId - blank Session captured before the Settings write.
     * @param id - the effective default that the write persisted.
     * @returns the Host refusal text, or undefined when applied or no longer relevant.
     */
    syncBlankSession(expectedSessionId: SessionSummary['id'], id: string): Promise<string | undefined>;
    /** Acknowledge the introduction cue once the chip has played it. */
    introduced(): void;
    /**
     * Hand the staged choice to the current session, if there is one to take it.
     *
     * Called both by `select()` and by whoever observes the current session
     * changing, because the session may appear either before or after the pick.
     * List updates do not repeat a selection while its response is pending.
     * @returns this attempt's Host refusal, or undefined when successful or no switch starts.
     */
    apply(): Promise<string | undefined>;
}
//# sourceMappingURL=seat-store.d.ts.map