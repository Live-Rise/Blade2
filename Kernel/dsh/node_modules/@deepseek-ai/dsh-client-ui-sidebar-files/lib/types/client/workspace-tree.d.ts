/**
 * The workspace navigator's input to the `sidebar.files.tree` Factory: a pane's
 * listed levels as the tree's flat entries and directory states.
 *
 * The store keys a level by absolute path and a child by `childPath`, which
 * joins the parent with `/` after dropping its trailing separators, so every
 * key below the root is already a tree key. Only the root differs: its tree
 * key drops its trailing separators too, so `/` becomes the empty key and a
 * Windows drive root `C:\` becomes `C:`, and a top-level child's key still
 * ends at its parent's last `/`.
 *
 * Unless the reader shows them, hidden entries are left out, and a directory
 * whose every entry is hidden says so. The same conversion draws a search's
 * results, whose directories hold only the matches.
 */
import type { RemoteFailure } from '@deepseek-ai/dsh-api-remotes/client';
import type { TreeDirectoryState, TreeEntry } from './FileTree.tsx';
import type { TranslateNS } from '@deepseek-ai/dsh-client-ui-slots';
import { type FilesTreeState } from './store.ts';
/**
 * Say why a directory could not be listed, in terms of the directory.
 * @param t - namespace-bound translate.
 * @param failure - the settled Remote failure.
 * @returns the line to show under the directory.
 */
export declare function failureLine(t: TranslateNS<'sidebarFiles'>, failure: RemoteFailure): string;
/**
 * The tree key of a workspace root.
 * @param root - the root's absolute path as the Host reports it.
 * @returns the path without trailing separators.
 */
export declare function treeRootOf(root: string): string;
/**
 * The store key of the directory a tree key's entry is listed in.
 * @param tree - the pane's tree.
 * @param path - an entry's tree key.
 * @returns the absolute path of its parent, the root spelled as the store keeps it.
 */
export declare function parentOf(tree: FilesTreeState, path: string): string;
/**
 * Every listed entry of a pane's tree, collapsed levels included, each named
 * as its listing spells it. Unless hidden entries are drawn, the kept levels
 * below a hidden directory are left out with it, so a directory listed while
 * hidden entries were shown is not drawn again through its descendants.
 * @param levels - the pane's level states.
 * @param showHidden - whether hidden entries are drawn.
 * @returns the entries keyed as the tree draws them.
 */
export declare function workspaceEntries(levels: FilesTreeState['levels'], showHidden: boolean): TreeEntry[];
/**
 * Each directory's listing state: loading until its first listing settles, then
 * its latest failure, whether its listing is truncated, and whether it holds
 * only hidden entries that are not drawn.
 * @param tree - the pane's root, level states, and expanded directories.
 * @param t - namespace-bound translate, for failure lines.
 * @param showHidden - whether hidden entries are drawn.
 * @returns the states keyed as the tree draws them.
 */
export declare function workspaceDirectories(tree: Pick<FilesTreeState, 'root' | 'levels' | 'expanded'>, t: TranslateNS<'sidebarFiles'>, showHidden: boolean): Record<string, TreeDirectoryState>;
/**
 * Each directory of a search's results, which holds only the matches below it,
 * so a matched directory without matched descendants draws no empty line.
 * @param tree - the results as a tree.
 * @returns the states keyed as the tree draws them.
 */
export declare function searchDirectories(tree: Pick<FilesTreeState, 'root' | 'levels'>): Record<string, TreeDirectoryState>;
//# sourceMappingURL=workspace-tree.d.ts.map