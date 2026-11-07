import type { UserTextRule } from '@deepseek-ai/dsh-client-ui-conversation/client';
declare module '@deepseek-ai/dsh-client-ui-conversation/client' {
    interface UserTextDataMap {
        quote: {
            readonly text: string;
        };
    }
}
/** Recognize blockquote lines and display adjacent paragraph separators as inline spaces. */
export declare const quoteRule: UserTextRule<'quote'>;
//# sourceMappingURL=quote-rule.d.ts.map