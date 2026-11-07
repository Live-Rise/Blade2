import type { Context } from '@deepseek-ai/cordis';
import type { FileActions } from '../types.ts';
import { type FileActionKey } from './locales.ts';
export type { FileActions, FileActionSource, FileSaveResult } from '../types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        fileActions: FileActions;
    }
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Original-file action labels and feedback. */
        fileActions: FileActionKey;
    }
}
/** The native adapter is optional; Web uses its own download and save capabilities. */
export declare const inject: never[];
/** @param ctx - plugin-owned Client context. */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map