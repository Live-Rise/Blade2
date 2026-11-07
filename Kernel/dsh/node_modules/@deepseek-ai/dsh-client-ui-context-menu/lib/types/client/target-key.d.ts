/** Stable equality for the JSON-compatible target vocabulary, independent of object insertion order. */
import type { ContextMenuTarget } from './types.ts';
/**
 * Compare captured JSON targets independently of object key insertion order.
 * @param target - captured immutable target.
 * @returns canonical JSON identity.
 */
export declare function menuTargetKey(target: ContextMenuTarget): string;
//# sourceMappingURL=target-key.d.ts.map