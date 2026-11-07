/**
 * Per-message feedback controls: the Like/Dislike pair inside the assistant
 * message's IconActions row, between copy and branch. Either rating opens the
 * Session's feedback dialog, whose submission records that judgment with its
 * category and text. Clicking the recorded rating retracts it. A recorded rating
 * shows the filled glyph so the signal survives a pointer leaving the row.
 * @module @deepseek-ai/dsh-client-ui-message-feedback/client/MessageFeedbackActions
 */
import type { MessageFeedbackActionProps } from './slots.ts';
/**
 * One message's feedback controls, rendered only while the current Peer may
 * record message feedback in this Session.
 * @param props - the owner's message identity, the injected verbs, the
 * shared feedback hook, and the Session access Hook.
 * @returns the rating buttons with any failure notice beside them, or nothing without access.
 */
export declare function MessageFeedbackActions(props: MessageFeedbackActionProps): import("react").JSX.Element | null;
//# sourceMappingURL=MessageFeedbackActions.d.ts.map