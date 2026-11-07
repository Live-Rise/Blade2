/** Non-step event rows: request admission, prompt/tool changes, and model-free prunes. */
import type { ConversationLocation, ConversationNodeDefinition } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { StatsEventMetrics } from './stats-contract.ts';
/** Event shape a Conversation Definition receives. */
type MatchEvent = Parameters<ConversationNodeDefinition['match']>[0];
/**
 * Turn and step a Location resolves.
 * @param location - engine Location of a match.
 * @returns the resolved turn and step, or null when the window lost them.
 */
export declare function locationTurnStep(location: ConversationLocation | undefined): {
    readonly turn: number;
    readonly step: number | null;
} | null;
/**
 * Normalize one non-step event into row values.
 * @param event - a session event seen by the Definition match.
 * @param location - engine Location of the event, for turn and step.
 * @returns row metrics, or null for events the table does not explain.
 */
export declare function eventMetrics(event: MatchEvent, location: ConversationLocation | undefined): StatsEventMetrics | null;
export {};
//# sourceMappingURL=stats-event.d.ts.map