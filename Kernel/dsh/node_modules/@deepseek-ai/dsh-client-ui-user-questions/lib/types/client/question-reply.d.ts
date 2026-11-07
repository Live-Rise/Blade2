import type { ConversationNodeDefinition } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
/** One asked question as echoed in the reply payload. */
export interface QuestionReplyQuestion {
    readonly id: string;
    readonly question: string;
    readonly header?: string;
}
/** One structured answer as echoed in the reply payload. */
export interface QuestionReplyAnswer {
    readonly id: string;
    readonly selected: readonly string[];
    readonly custom?: string;
}
/** Presentation data of one submitted answer batch. */
export interface QuestionReplyData {
    readonly callId: string;
    /** A submitted answer batch, including individually skipped questions. */
    readonly outcome: 'answered';
    readonly questions: readonly QuestionReplyQuestion[];
    readonly answers: readonly QuestionReplyAnswer[];
    /** Model-facing text, shown when the payload is unreadable. */
    readonly text: string;
    readonly time: number;
}
declare module '@deepseek-ai/dsh-client-ui-chat/client' {
    interface ChatNodeDataMap {
        /** One answered question batch, regardless of foreground wait timing. */
        'question-reply': QuestionReplyData;
    }
}
interface QuestionReplyState extends QuestionReplyData {
    readonly seq: number;
}
/**
 * Read the question and answer pairs out of the reply text at the conversation boundary.
 * @param text - Model-facing JSON payload of the steered message.
 * @returns The pairs, or empty lists when the payload is unreadable.
 */
export declare function replyPairsOf(text: string): Pick<QuestionReplyData, 'questions' | 'answers'>;
/**
 * Answer values of one question in display order: the selected option labels
 * followed by a non-blank custom answer.
 * @param data - Projected reply holding the recorded answers.
 * @param id - Question id to read.
 * @returns The values, empty when the user skipped that question.
 */
export declare function replyAnswerValues(data: QuestionReplyData, id: string): string[];
/**
 * Clipboard text of one answer batch: every question with the answer the user
 * gave, without question details or unselected options.
 * @param data - Projected reply to copy.
 * @param t - Bound `question` namespace translator owning the answer labels.
 * @returns One block per question, or the model-facing text when the payload was unreadable.
 */
export declare function replyClipboardText(data: QuestionReplyData, t: PropsLocale<'question'>['t']): string;
/** Reply data retained until a matching tool result records a complete answer batch. */
interface AnsweredQuestionState {
    readonly questions: readonly QuestionReplyQuestion[] | null;
    readonly reply: QuestionReplyState | undefined;
}
/**
 * In-time ask-user answers paired by native callId or PTC subCallId. Pending, failed, and
 * malformed results retain their generic Tool presentation; late answers are
 * owned by questionReplyDefinition. Work is bounded by this call's answer batch.
 */
export declare const answeredQuestionDefinition: ConversationNodeDefinition<AnsweredQuestionState>;
/**
 * Late-reply projection owned by the questions UI. It claims the reply message
 * on the Chat target, so the generic message Definition never presents it.
 */
export declare const questionReplyDefinition: ConversationNodeDefinition<QuestionReplyState>;
export {};
//# sourceMappingURL=question-reply.d.ts.map