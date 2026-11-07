/** Bind generated Remote descriptors to the source-tested browser lifecycle. */
import type { Context } from '@deepseek-ai/cordis';
export { inject } from './mount.ts';
export type { ActionCardBodyProps, ActionCardDescriptor, ActionCardRegistry, FoldedTextProps } from './registry.ts';
/**
 * Mount action cards and their Remote.
 * @param ctx - Client plugin context.
 * @returns disposer for every contribution.
 */
export declare function apply(ctx: Context): Promise<() => Promise<void>>;
//# sourceMappingURL=index.d.ts.map