/** Application submenus for captured Sessions, shared by header and row menus. */
import type { Context } from '@deepseek-ai/cordis';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { OpenInAppController } from './controller.ts';
import type { OpenInAppToastSource } from './open-toast.tsx';
/**
 * Share directory availability, successful choices, and launch feedback with the file-tree control.
 * @param ctx - owning plugin context.
 * @param controller - directory application catalog and launcher.
 * @param t - localized action labels.
 * @param report - app-wide native operation feedback; launch failures resolve without registry feedback.
 */
export declare function registerSessionOpenWith(ctx: Context, controller: OpenInAppController, t: PropsLocale<'open-in-app'>['t'], report: OpenInAppToastSource['report']): void;
//# sourceMappingURL=session-open-with.d.ts.map