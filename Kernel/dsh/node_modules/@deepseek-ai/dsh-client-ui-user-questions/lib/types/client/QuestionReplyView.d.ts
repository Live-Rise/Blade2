import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { QuestionReplyData } from './question-reply.ts';
type QuestionReplyViewProps = PropsRuntime<'conversation.chat.node', 'question-reply'> & PropsLocale<'question'>;
/** Render recorded questions and answers without an expandable process row. */
export declare const QuestionReplyView: import("react").MemoExoticComponent<({ node, t }: QuestionReplyViewProps) => import("react").JSX.Element>;
/**
 * Render one answered batch with questions above their corresponding answers.
 * @param props - Reply data and the question locale translator.
 * @returns The right-aligned, fully visible question-and-answer bubble.
 */
export declare function QuestionReplyBubble({ data, t }: {
    data: QuestionReplyData;
    t: PropsLocale<'question'>['t'];
}): import("react").JSX.Element;
export {};
//# sourceMappingURL=QuestionReplyView.d.ts.map