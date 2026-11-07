/** Original-file menu contributions owned by the file-action capability. */
import type { Context } from '@deepseek-ai/cordis';
import type { FileActions } from '../types.ts';
import type { FileActionKey } from './locales.ts';
/**
 * Register original-byte operations without requiring an application-opening plugin.
 * @param ctx - menu and authorized file-read services.
 * @param actions - file-action owner.
 * @param t - localized labels and feedback.
 */
export declare function registerFileMenus(ctx: Context, actions: FileActions, t: (key: FileActionKey) => string): void;
//# sourceMappingURL=context-menus.d.ts.map