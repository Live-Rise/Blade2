import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
/** Framework command data and the owning Team dictionary. */
export type TeamCommandRowProps = PropsRuntime<'conversation.chat.commandview'> & PropsLocale<typeof NS>;
/**
 * Render the recorded Team command with the shared agent glyph and disclosure.
 * @param props - command record and localized Team text.
 * @returns an outcome row; recorded failures and multiline output remain expandable.
 */
export declare function TeamCommandRow({ node, t }: Pick<TeamCommandRowProps, 'node' | 't'>): import("react").JSX.Element;
//# sourceMappingURL=TeamCommandRow.d.ts.map