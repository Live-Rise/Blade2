import type { Volatile } from '@deepseek-ai/cordis';
import type { ThemePreference } from './theme-settings.ts';
import z from '@deepseek-ai/schemastery';
import type { Context } from '@deepseek-ai/cordis';
export { DEFAULT_FONT_SIZE, DEFAULT_PREFERENCE, FONT_SIZE_FIELD, FONT_SIZE_MAX, FONT_SIZE_MIN, THEME_PREFERENCE_FIELD, THEME_PREFERENCES, THEME_SETTINGS_NAMESPACE, type ThemePreference, type ThemeSettings, } from './theme-settings.ts';
/** Runtime preferences projected to the browser. */
export interface Config {
    /** Browser palette preference. */
    preference: Volatile<ThemePreference>;
    /** Browser font size in pixels. */
    fontSize: Volatile<number>;
}
/** Live theme and typography preferences. */
export declare const Config: z<Schemastery.ObjectS<NoInfer<{
    preference: z<"light" | "dark" | "system", "light" | "dark" | "system", "volatile-defined">;
    fontSize: z<number, number, "volatile-defined">;
}>>, Schemastery.ObjectT<NoInfer<{
    preference: z<"light" | "dark" | "system", "light" | "dark" | "system", "volatile-defined">;
    fontSize: z<number, number, "volatile-defined">;
}>>, "plain">;
/** Supply the current palette before browser plugins start.
 * @param ctx Host plugin context.
 * @param config Validated live theme preferences.
 */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map