/** Temporary 1017 badge for the two official DeepSeek model routes. */
import type { Context } from '@deepseek-ai/cordis';
import { type LabelKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** The temporary model release badge. */
        'deepseek-model-label-1017': LabelKey;
    }
}
/** Required services: model-label registration and localized badge text. */
export declare const inject: string[];
/**
 * Decorate existing model labels while their declaring slot is mounted.
 * @param ctx - Client plugin context owning the dictionaries and registration.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map