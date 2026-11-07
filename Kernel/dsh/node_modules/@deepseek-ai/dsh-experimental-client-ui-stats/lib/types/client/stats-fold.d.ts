/** Event folding and per-step metric derivation for the Statistics table. */
import type { SessionEventLike } from '@deepseek-ai/dsh-api-session-controller/client';
import type { ImageAttachmentRef } from '@deepseek-ai/dsh-attachment';
import type { UserMessage } from '@deepseek-ai/dsh-llm';
import type { StatsCall, StatsStepMetrics, StatsStepState } from './stats-contract.ts';
/** Calls listed per step. */
export declare const CALL_LIMIT = 12;
/**
 * Whether one tool name is the Code-Mode runner; its real tools run inside it.
 * @param name - tool name, or null for a nameless call.
 * @returns true when the name is the Code-Mode runner.
 */
export declare function isCodeMode(name: string | null): boolean;
/**
 * Sender agent name of a teammate/agent notification, parsed from its body prefix.
 * @param message - waking user message stamped with its producer source.
 * @returns the readable member name, or null when the frame omits it or carries an opaque session id.
 */
export declare function triggerSenderOf(message: UserMessage): string | null;
/**
 * Initial state of one step or one turn-level marker.
 * @param turn - turn number.
 * @param step - step number, or the marker's pseudo step.
 * @param mark - marker kind; steps pass null.
 * @returns the empty state.
 */
export declare function newState(turn: number, step: number, mark?: StatsStepState['mark']): StatsStepState;
/**
 * User question: only a human-typed `user` source is the text the user wrote.
 * @param message - one user-role message from the Session log.
 * @returns the one-line question and the durable images it attached.
 */
export declare function promptOf(message: UserMessage): {
    text: string | null;
    images: readonly ImageAttachmentRef[];
};
/**
 * Fold one session event into the step state. Events other than `step/start`
 * create a state when the window starts mid-step, so context and throughput
 * still show while start-dependent metrics stay unrecorded.
 * @param state - current state, or undefined before the first foldable event.
 * @param event - one durable or client-only session event.
 * @returns the next state, or undefined when the event needs a start it lacks.
 */
export declare function foldEvent(state: StatsStepState | undefined, event: SessionEventLike): StatsStepState | undefined;
/**
 * Wall span covered by calls that both started and settled; overlaps count once.
 * @param calls - calls with their paired start and end times.
 * @returns the span in milliseconds, or null when no call is paired.
 */
export declare function toolSpanUnion(calls: readonly StatsCall[]): number | null;
/**
 * Measured metrics of one step.
 * @param state - folded step state.
 * @returns the row values the table renders.
 */
export declare function stepMetrics(state: StatsStepState): StatsStepMetrics;
/**
 * State rebuilt from the matches a window still holds when its state is absent.
 * Event order is preserved, so `step/start` and `step/end` restore the
 * boundaries whenever they survived.
 * @param matches - accepted matches in ascending sequence order.
 * @returns the folded state, or undefined when nothing was foldable.
 */
export declare function fallbackState(matches: readonly {
    readonly event: SessionEventLike;
}[]): StatsStepState | undefined;
//# sourceMappingURL=stats-fold.d.ts.map