/**
 * Synthetic closer events that balance a session log whose tail turn is open.
 * Two producers share the mechanism: crash recovery closes an interrupted
 * persisted log on reload, and fork-seed construction closes a prefix cut
 * inside the source's open turn. Both preserve every fully written event and
 * close the unfinished step and turn. Calls in already closed steps remain
 * unchanged, including any missing results.
 * @module @deepseek-ai/dsh-session/repair
 */
import type { SessionEvent } from './types.ts';
/** Recovery code for an assistant tool request that never reached a recorded call start. */
export declare const TOOL_NOT_STARTED = "TOOL_NOT_STARTED";
/** Recovery code for a recorded tool call whose completed outcome was not durably recorded. */
export declare const TOOL_OUTCOME_UNKNOWN = "TOOL_OUTCOME_UNKNOWN";
/**
 * Why an open tail turn is closed with synthetic events: `interrupted` is
 * crash recovery over a persisted log; `forked` is a fork seed cut inside the
 * source's open turn. The cause selects the synthetic `turn/end` reason, the
 * model-visible wording of synthetic error tool results, and the
 * deterministic synthetic message-id prefix. The error codes
 * ({@link TOOL_NOT_STARTED} / {@link TOOL_OUTCOME_UNKNOWN}) are shared: both
 * causes state the same fact about the call's recorded lifecycle.
 */
export type OpenTurnCloseCause = {
    readonly kind: 'interrupted';
} | {
    readonly kind: 'forked';
};
/**
 * Return deterministic synthetic events that close an open tail turn. Unmatched
 * calls in its open step receive error results, followed by `step/end` and a
 * `turn/end` carrying the cause's reason. Calls in closed steps remain unchanged.
 * Sequences continue the log and timestamps reuse the last real event. A balanced or empty log returns no
 * events.
 *
 * Package-internal: each cause has exactly one owner, so external callers go
 * through {@link interruptedTurnClosers} (persistence crash recovery) or
 * `buildForkSeed` in `./fork.ts` (fork seeds) instead of selecting a cause.
 *
 * @param events - the log to scan: a valid committed prefix, possibly ending
 *   inside an open turn (a crash tail or a mid-turn fork cut).
 * @param cause - why the turn is being closed; selects the `turn/end` reason
 *   and the model-visible wording of synthetic error tool results.
 * @returns the synthetic closer events to append after `events`, in order; empty when the log is already balanced.
 */
export declare function openTurnClosers(events: readonly SessionEvent[], cause: OpenTurnCloseCause): SessionEvent[];
/**
 * Crash-recovery entry point: synthetic closers that balance a persisted log
 * whose tail turn was interrupted. Used by crash-recovery callers; fork
 * seeds receive their `forked`-cause closers through `buildForkSeed` in
 * `./fork.ts`, and cause selection stays internal to those two owners.
 *
 * @param events - the persisted log to scan, possibly ending inside an open turn.
 * @returns the synthetic `interrupted` closer events to append after `events`; empty when the log is already balanced.
 */
export declare function interruptedTurnClosers(events: readonly SessionEvent[]): SessionEvent[];
//# sourceMappingURL=repair.d.ts.map