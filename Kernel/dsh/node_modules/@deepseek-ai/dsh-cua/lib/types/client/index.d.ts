/** Bind the source-owned permission UI to its generated Remote contribution. */
import type { Context } from '@deepseek-ai/cordis';
export { inject } from './mount.ts';
/**
 * Activate the optional Computer Use permission controls.
 * @param ctx Browser plugin context.
 * @returns Cleanup of the UI and Remote contribution.
 */
export declare function apply(ctx: Context): Promise<() => Promise<void>>;
//# sourceMappingURL=index.d.ts.map