/**
 * Opt-in request-preparation tmux-location context. Eligible step attempts
 * append durable, source-attributed context naming the tmux session, window,
 * and pane this agent process runs in, plus the window's pane-tree layout.
 *
 * The plugin pulls state once per turn, for the first request (`step === 1`), by
 * running one `tmux display-message` through the `ctx.shell` executor service. It
 * confirms this process genuinely runs inside the pane `$TMUX_PANE` names by
 * matching the pane's `#{pane_tty}` against this process's controlling terminal,
 * so a terminal that merely inherited `$TMUX`/`$TMUX_PANE` from a tmux ancestor
 * (e.g. a VS Code integrated terminal) reads as "not in tmux". It re-injects
 * only when the rendered tmux state changes since the last injection (a moved,
 * renamed, or re-laid-out pane), with an optional `refreshIntervalMs` floor
 * between injections. Absent tmux environment, an inherited-only environment,
 * absent `ctx.shell`, or a failed query is a no-op, never an error: an executor
 * rejection is contained and logged as a warning so the turn continues.
 *
 * @module @deepseek-ai/dsh-tmux-context
 */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { ContextFormed } from '@deepseek-ai/dsh-llm';
declare module '@deepseek-ai/dsh-llm' {
    interface MessageSourceMap {
        /** Location attribution; readers preserve the content without this producer.
         * Its projection uses the kind to avoid repeated injection.
         * @persistenceAttribution
         */
        'tmux-context': {
            kind: 'tmux-context';
        } & ContextFormed;
    }
}
/** Cordis plugin name used by loader diagnostics. */
export declare const name = "tmux-context";
/** The agent registry that owns pre-step processing. */
export declare const inject: string[];
/** Per-turn tmux-location scheduling. Invalid values fail plugin load. */
export interface Config {
    /** Minimum milliseconds between durable injections in one session. Omit or set to 0 to inject on every eligible change. */
    refreshIntervalMs?: number;
}
/** Schemastery validation for {@link Config}. */
export declare const Config: z<Config>;
/**
 * Register a prepended pre-step listener for the lifetime of `ctx`.
 * Steps admitted during activation await the projection registration; a
 * registration failure or owner disposal prevents the request from proceeding.
 * @param ctx - plugin context; the listener is disposed with it.
 * @param config - durable refresh scheduling configuration.
 * @returns when the listener and projection are ready.
 * @throws when the refresh interval is invalid.
 */
export declare function apply(ctx: Context, config: Config): Promise<void>;
//# sourceMappingURL=index.d.ts.map