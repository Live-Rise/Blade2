/**
 * Document icon sprite: one hidden `<svg>` whose `<symbol>` children carry
 * every product icon, so ui-primitives icons render through `<use>` and
 * inherit `currentColor` and CSS variables at each use site.
 */
import type { Context } from '@deepseek-ai/cordis';
import { type IconSymbolId } from '@deepseek-ai/dsh-client-ui-primitives';
/** Icon replacements keyed by sprite symbol id; see `ThemeDefinition.icons`. */
export type IconReplacements = Readonly<Partial<Record<IconSymbolId, string>>>;
/**
 * Mount the sprite and keep each symbol on the active theme's replacement or
 * the ui-primitives default; plugin disposal removes the sprite.
 * @param ctx - owning plugin context; `theme/change` snapshots drive updates.
 * @param icons - replacements of the theme active at install time.
 */
export declare function installIconSprite(ctx: Context, icons: IconReplacements | undefined): void;
//# sourceMappingURL=icon-sprite.d.ts.map