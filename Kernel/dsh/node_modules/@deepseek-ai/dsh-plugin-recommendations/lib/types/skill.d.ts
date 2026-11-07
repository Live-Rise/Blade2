import type { Context } from '@deepseek-ai/cordis';
import type { BundleInfo } from '@deepseek-ai/dsh-plugin-manager/types';
/** Shipped optional plugins only; no arbitrary install target is accepted.
 * @param bundle - a profile bundle as the plugin manager reports it.
 * @returns whether a card may offer and enable it.
 */
export declare function eligible(bundle: BundleInfo): boolean;
/** Register the experiment's skill without adding a model tool or a system-prompt section.
 * @param ctx - Owning plugin context with skill and plugin registries.
 */
export declare function registerOfficialPluginsSkill(ctx: Context): void;
//# sourceMappingURL=skill.d.ts.map