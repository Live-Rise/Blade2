import type { DirLevel, FilesTreeState } from './store.ts';
/** One entry a search matched. */
export interface SearchMatch {
    /** `/`-separated path relative to the workspace root, without leading or trailing `/`. */
    readonly path: string;
    /** Whether the entry is a file, a directory, or something else. */
    readonly type: DirLevel['entries'][number]['type'];
}
/**
 * Draw search results as a fully expanded tree.
 *
 * Every directory the results pass through gets a ready level: the root, each
 * result's ancestors, and each result that is itself a directory. A level lists
 * the children the results put there, once each, in first-seen order; a name
 * that is an ancestor anywhere is a directory. A directory that matched but
 * holds no matching descendant has a level with no entries, which the
 * navigator draws differently from an empty listed directory. Every level is
 * expanded, no level is truncated, and the tree does not refresh itself.
 * @param root - absolute path of the workspace root, as `FilesTreeState.root` holds it.
 * @param entries - the search's entries.
 * @returns the tree to draw.
 */
export declare function buildSearchTree(root: string, entries: readonly SearchMatch[]): FilesTreeState;
/**
 * Where the query occurs in one name, for highlighting.
 *
 * Matching ignores case by Unicode simple case folding; the ranges index the
 * name as given. An empty query has no occurrence.
 * @param name - the drawn name.
 * @param query - the search text.
 * @returns non-overlapping `[start, end)` UTF-16 ranges, left to right.
 */
export declare function matchRanges(name: string, query: string): readonly (readonly [number, number])[];
//# sourceMappingURL=search-tree.d.ts.map