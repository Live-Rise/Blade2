import type { PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { Context } from '@deepseek-ai/cordis';
import type { OpenInAppController } from './controller.ts';
import type { OpenInAppPathController } from './open-path.ts';
import type { OpenInAppKey } from './locales.ts';
import type { OpenInAppToastSource } from './open-toast.tsx';
/**
 * Register native opening, original-path reveal, and URL actions.
 * @param ctx - declared plugin dependencies.
 * @param paths - existing native file actions.
 * @param t - localized action labels.
 * @param directories - installed directory applications.
 * @param report - native operation feedback shared with the application controls.
 */
export declare function registerContextMenus(ctx: Context, paths: OpenInAppPathController, t: (key: OpenInAppKey, values?: Record<string, string>) => string, directories: OpenInAppController, report: OpenInAppToastSource['report']): void;
/** @param props - icon metadata captured with the resolved action. @returns Application image or the native-open glyph. */
export declare function ContextMenuApplicationIcon({ icon }: Pick<PropsRuntime<'contextMenu.icon'>, 'icon'>): import("react").JSX.Element;
//# sourceMappingURL=context-menus.d.ts.map