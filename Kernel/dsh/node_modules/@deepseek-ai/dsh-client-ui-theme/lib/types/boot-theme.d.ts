/**
 * Theme bootstrap row for the browser's pre-plugin interval. Each index
 * render embeds the current durable built-in preference and content font size.
 * Head CSS colors the document canvas before script execution; the body script
 * installs the palette selector and font size that the client presenters adopt.
 */
import type { IndexInjection } from '@deepseek-ai/dsh-host-webserver';
import { type ThemePreference } from './theme-settings.ts';
/**
 * Theme bootstrap rows: head CSS colors the document canvas before
 * first paint, then the body script installs the palette selector and font
 * size before the shell mount and module script.
 * @param preference - Current Host-backed built-in preference.
 * @param fontSize - Current Host-backed content font size in px.
 * @returns head and body script rows in execution order.
 */
export declare function bootThemeInjections(preference?: ThemePreference, fontSize?: number): IndexInjection[];
//# sourceMappingURL=boot-theme.d.ts.map