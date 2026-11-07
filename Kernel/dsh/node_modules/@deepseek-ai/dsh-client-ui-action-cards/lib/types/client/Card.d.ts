import type { HostObservable, InjectFace, PropsLocale, PropsRuntime, Translate } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { ToolCallId } from '@deepseek-ai/dsh-llm/brand';
import type { ActionCardDecision, ActionCardFields, ActionCardProposal, ActionCardResult } from '@deepseek-ai/dsh-action-cards/types';
import type { FoldedTextProps, RegisteredBody } from './registry.ts';
/** Session-bound actions and registered bodies, without a service in the component. */
export interface TailInjected {
    /**
     * Send one user action for a card of the displayed Session.
     * @param callId - the card's call id.
     * @param verb - `dismiss` or one of the kind's verbs.
     * @param payload - kind-defined fields.
     * @returns the Host result, or undefined when the reply was lost.
     */
    act: (callId: ToolCallId, verb: string, payload?: ActionCardFields) => Promise<ActionCardResult | undefined>;
    /**
     * Bind a body's own locale namespace.
     * @param locale - namespace the body declared.
     * @returns its translator.
     */
    bind: (locale: string) => Translate;
    hooks: {
        bodies: HostObservable<ReadonlyMap<string, RegisteredBody>>;
    };
    sessionId: SessionId;
}
type TailProps = Pick<PropsRuntime<'conversation.chat.turnTail'>, 'turn' | 'useProjection'> & PropsLocale<'actionCards'> & InjectFace<TailInjected>;
/**
 * Fold omitted text without measuring layout; a wide grapheme counts twice so Chinese and English fold near the
 * same line. The full text is never altered.
 * @param props - text and its localized toggle labels.
 * @returns the paragraph and, when text was omitted, its toggle.
 */
export declare function FoldedText({ text, expandLabel, collapseLabel }: FoldedTextProps): import("react").JSX.Element;
/**
 * Place the card admitted in a turn after that turn's final answer, once the turn has ended with one.
 * @param props - Turn location, projection reader, registered bodies and Session-bound actions.
 * @returns the card, or null while the turn is open or when it ended without answer text.
 */
export declare function ActionCardTail({ turn, useProjection, ...props }: TailProps): import("react").JSX.Element | null;
/**
 * Render the frame around one card.
 * @param props - the card, its state, registered bodies and actions.
 * @returns the card section.
 */
export declare function ActionCard({ proposal, decision, sessionId, act, bind, useBodies, t }: {
    proposal: ActionCardProposal;
    decision: ActionCardDecision | undefined;
} & InjectFace<TailInjected> & PropsLocale<'actionCards'>): import("react").JSX.Element;
export {};
//# sourceMappingURL=Card.d.ts.map