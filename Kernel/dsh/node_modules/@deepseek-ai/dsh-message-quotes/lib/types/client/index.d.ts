/** Quote selection actions and literal UserText excerpts registered by the browser plugin. */
import type { Context } from '@deepseek-ai/cordis';
import { type MessageQuotesKey } from './locales.ts';
export type {} from './quote-rule.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Quote selection action copy. */
        messageQuotes: MessageQuotesKey;
    }
}
/** Browser plugin name. */
export declare const name = "message-quotes";
/** Services needed to admit selections and register Quote presentation. */
export declare const inject: string[];
/**
 * Register Quote admission, its action surface, and literal UserText rendering.
 * @param ctx - Client root context owning every Quote contribution.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map