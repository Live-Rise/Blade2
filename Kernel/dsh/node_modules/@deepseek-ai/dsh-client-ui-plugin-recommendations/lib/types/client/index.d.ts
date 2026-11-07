/** Bind the generated Remote contribution to the source-tested browser lifecycle. */
import type { Context } from '@deepseek-ai/cordis';
export { inject } from './mount.ts';
/** Mount recommendation UI and its optional Remote.
 * @param ctx - Client plugin context.
 * @returns disposer for both contributions.
 */
export declare function apply(ctx: Context): Promise<() => Promise<void>>;
//# sourceMappingURL=index.d.ts.map