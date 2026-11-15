/**
 * The file tree's view state, one record per right-Sidebar pane: which
 * directories are expanded, what each loaded level contains, how the
 * navigator is shown, and the pane's search. A Slot-standard exclusive store
 * (one instance per Session) keyed by pane, so every file tab of a pane draws
 * the same tree; the face decides when a pane's record is forgotten
 * (`FilesInjected.attach`). Beside it live pure reads shared by the face and
 * the drawings: path keys, where a tab's file sits in the listed tree, and the
 * navigator's geometry.
 */
import { type EngineStoreHandle } from '@deepseek-ai/dsh-client-store';
import type { RemoteFailure } from '@deepseek-ai/dsh-api-remotes/client';
import type { PaneId } from '@deepseek-ai/dsh-client-ui-dockkit';
import type { WorkspaceDirectoryEntry } from '@deepseek-ai/dsh-api-workspace-files/types';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
/** The navigator's width before a pane has a preference, in CSS pixels. */
export declare const DEFAULT_NAVIGATOR_WIDTH = 260;
/**
 * The row width from which a preview's navigator takes its width from the
 * body instead of floating over it. Fixed, so resizing the tree never moves
 * the navigator between the two layouts.
 */
export declare const PUSH_FROM = 560;
/**
 * Fit the navigator to the row it shares with the preview. In a row at least
 * `PUSH_FROM` wide the navigator sits beside the body, so it is at most the
 * row less its card's margins and `PREVIEW_MINIMUM`; in a narrower row it
 * floats over the body and is at most the row less `ROW_REMAINDER`.
 * @param available - the row's width in CSS pixels; `undefined` before it is measured.
 * @param width - the pane's width preference.
 * @returns the resize bounds and the width drawn.
 */
export declare function navigatorGeometry(available: number | undefined, width: number): {
    readonly minimum: number;
    readonly maximum: number;
    readonly width: number;
};
/**
 * One directory's contents, as one expanded level of the tree.
 *
 * The endpoint's listing also names the directory as a workspace-relative path;
 * the tree keys every level by absolute path instead, so the adapter drops it.
 */
export interface DirLevel {
    /** The directory's entries, in the endpoint's order. */
    readonly entries: readonly WorkspaceDirectoryEntry[];
    /** The listing hit the endpoint's entry cap, so entries are missing. */
    readonly truncated: boolean;
}
/**
 * What one directory level is doing right now. A `ready` level marked
 * `refreshing` keeps its last listing until the next one settles: the listing
 * in flight, or, once its directory opens again, the first listing there. An
 * entry missing from it is not yet known to be absent. A directory whose
 * listing fails also clears the mark on every level below it, which it does
 * not list.
 */
export type LevelState = {
    readonly kind: 'loading';
} | {
    readonly kind: 'ready';
    readonly level: DirLevel;
    readonly failure?: RemoteFailure;
    readonly refreshing?: true;
} | {
    readonly kind: 'failed';
    readonly failure: RemoteFailure;
};
/**
 * One pane's tree: its root, the levels it has asked for, and what is open.
 *
 * Every path here is absolute: the root is the session's working directory as
 * the Host reports it, and a child is the parent joined with the entry name.
 */
export interface FilesTreeState {
    autoRefresh: boolean;
    /** Absolute path of the workspace root this tree is rooted at. */
    root: string;
    /** Level state by absolute directory path; a path absent here was never asked for. */
    levels: Record<string, LevelState>;
    /** Expanded absolute directory paths, root included. */
    expanded: string[];
}
/**
 * One settled search, as the pane draws it instead of the tree: the matches,
 * or why the Host could not search.
 */
export type FilesSearchResult = {
    readonly kind: 'found';
    /** The trimmed query the search answered, which the rows highlight. */
    readonly query: string;
    /** The matches as a tree; its `expanded` is the search's own and starts with every level. */
    readonly tree: FilesTreeState;
    /** How many entries the search returned. */
    readonly count: number;
    /** More entries matched than the Host returns, so only its first `count` are drawn. */
    readonly truncated: boolean;
    /**
     * Part of the workspace was not searched: the Host's search passed its file
     * limit, or a directory could not be read.
     */
    readonly incomplete: boolean;
    /** Whether the search was asked to leave out hidden entries; it does not say that any were. */
    readonly hiddenLeftOut: boolean;
    /** Whether the search was given at least one enabled exclusion rule; it does not say that a rule matched. */
    readonly excluded: boolean;
    /** Whether the search was asked to respect ignore files; it does not say that one ignored anything. */
    readonly ignored: boolean;
} | {
    readonly kind: 'failed';
    /** The trimmed query the failed search asked for. */
    readonly query: string;
    /** Why the Host could not search. */
    readonly failure: RemoteFailure;
};
/** One pane's search while its query is not blank. */
export interface FilesSearchState {
    /** A search is running; the last settled result stays drawn until its answer replaces it. */
    readonly pending: boolean;
    /** The last settled search; `undefined` until the pane's first search settles. */
    readonly result: FilesSearchResult | undefined;
}
/** One pane's navigator: whether and how wide it is shown, its tree once started, and its search. */
export interface FilesPaneState {
    /**
     * Whether a preview's navigator is shown; `undefined`, hidden, until the
     * pane's first choice. The `files` page shows its tree regardless.
     */
    open: boolean | undefined;
    /** The reader's width preference in CSS pixels; the navigator clamps it to the room it has. */
    width: number;
    /** The tree, listed once the navigator is first shown. */
    tree: FilesTreeState | undefined;
    /** The search field's text, as typed; not persisted. */
    query: string;
    /** Whether the files page draws its search bar; hiding it retains the query and results. */
    searchBarOpen: boolean;
    /** The pane's search, which the navigator draws instead of the tree; `undefined` while the query is blank. */
    search: FilesSearchState | undefined;
}
/** Every pane's navigator, keyed by pane id. */
export interface FilesState {
    byPane: Record<PaneId, FilesPaneState>;
}
/**
 * The absolute path of one child entry.
 *
 * Joined with `/` whatever the parent's separators: the Host resolves mixed
 * separators, and the tree only needs a stable key.
 * @param parent - absolute path of the listed directory.
 * @param name - the entry's basename.
 * @returns the child's absolute path.
 */
export declare function childPath(parent: string, name: string): string;
/** Where a file sits in the listed tree. */
export interface TreeFileLocation {
    /** Listed ancestor directories, outermost first, with the parent each is listed under. */
    readonly ancestors: readonly {
        readonly parent: string;
        readonly path: string;
    }[];
    /** The file's tree key, once every level on the way is listed and holds it. */
    readonly current: string | undefined;
    /** A level on the way is still being listed, or is listed again and lacks the next name. */
    readonly pending: boolean;
}
/**
 * Walk the listed levels to a workspace-relative file, keeping the listing's
 * spelling of each name; a Windows root also matches names regardless of case.
 * @param tree - the pane's tree.
 * @param relativePath - the file's path under the tree's root, `/`-separated.
 * @returns the ancestors found so far, the file's tree key, and whether a level is pending.
 */
export declare function locateFile(tree: FilesTreeState, relativePath: string): TreeFileLocation;
/**
 * The workspace-relative path of the file a tab shows, when it is a file of
 * this Session under the tree's root; dot segments are not resolved here. An
 * absolute path is under a Windows drive or UNC root regardless of case and
 * separator spelling, and under a POSIX root only with the root's exact case.
 * @param address - the tab's content address.
 * @param sessionId - the Session the tree lists.
 * @param root - the tree's root.
 * @returns the `/`-separated relative path, or `undefined` for anything else.
 */
export declare function relativeFileOf(address: string, sessionId: SessionId, root: string): string | undefined;
/** The tree store's write set; every action names the pane it writes. */
type FilesActions = {
    autoRefresh: (draft: FilesState, paneId: PaneId, enabled: boolean) => void;
    start: (draft: FilesState, paneId: PaneId, root: string) => void;
    loading: (draft: FilesState, paneId: PaneId, path: string) => void;
    loaded: (draft: FilesState, paneId: PaneId, path: string, level: DirLevel) => void;
    failed: (draft: FilesState, paneId: PaneId, path: string, failure: RemoteFailure) => void;
    toggled: (draft: FilesState, paneId: PaneId, path: string) => void;
    shown: (draft: FilesState, paneId: PaneId, open: boolean) => void;
    searchBarShown: (draft: FilesState, paneId: PaneId, open: boolean) => void;
    resized: (draft: FilesState, paneId: PaneId, width: number) => void;
    forget: (draft: FilesState, paneId: PaneId) => void;
    queried: (draft: FilesState, paneId: PaneId, query: string) => void;
    searching: (draft: FilesState, paneId: PaneId) => void;
    searched: (draft: FilesState, paneId: PaneId, result: FilesSearchResult) => void;
    searchToggled: (draft: FilesState, paneId: PaneId, path: string) => void;
};
/**
 * Declare the file tree's store.
 *
 * A factory rather than a shared handle: the registration declares it as an
 * exclusive store, so the framework mints one instance per session.
 * @returns the store handle to declare on the registration.
 */
export declare function createFilesStore(): EngineStoreHandle<FilesState, FilesActions>;
/** One Session's live instance of the tree's store, as the face reads and writes it. */
export type FilesStoreInstance = ReturnType<ReturnType<typeof createFilesStore>['create']>;
export {};
//# sourceMappingURL=store.d.ts.map