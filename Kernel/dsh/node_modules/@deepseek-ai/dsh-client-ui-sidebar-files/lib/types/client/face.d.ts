/**
 * The navigator's half outside React. The injected face (`filesFace`) lists
 * directories into the store through its own actions, reveals the tab's file,
 * carries scroll and keyboard focus between drawings of a pane's tree, runs
 * each pane's search, and measures the navigator's row. The component never
 * awaits or observes anything: it registers what it draws and calls the face.
 * The per-pane lifetimes are stated on `attach` and `hold`. A search asks the
 * Host whenever the query or the workspace's search preferences change; each
 * pane runs one search at a time, a newer one aborting the older, whose answer
 * is dropped. It writes its pending state and its result to the store, and the
 * results keep a scroll offset apart from the tree's. The workspace's search
 * preferences come from a book shared by every Session. Beside the face live
 * its inputs: the Remote listing and watch bindings, and `canOpenOf`; the
 * search's binding lives in `search-workspace.ts`.
 */
import type { ClientRemote, RemoteResult } from '@deepseek-ai/dsh-api-remotes/client';
import type { PaneId, TabId } from '@deepseek-ai/dsh-client-ui-dockkit';
import type { SidebarRightTabDefinition, SidebarRightTabNavigation } from '@deepseek-ai/dsh-client-ui-sidebar-right/client';
import type { HostObservable } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import { type DirLevel, type FilesStoreInstance } from './store.ts';
import type { WorkspaceSearchPreferenceBook } from './search-preferences.ts';
import { type WorkspaceSearchPreferences } from './search-rules.ts';
import type { SearchWorkspace } from './search-workspace.ts';
/**
 * Observe one directory without recursively watching its descendants.
 * @param sessionId - Session owning the directory tree.
 * @param path - absolute directory path.
 * @param signal - node lifetime.
 * @returns readiness and invalidation notifications.
 */
export type WatchWorkspaceDirectory = (sessionId: SessionId, path: string, signal: AbortSignal) => AsyncIterable<'ready' | 'change'>;
/**
 * Bind directory observation to the Remote stream supervisor.
 * @param remote - Client Remote with workspace file streams.
 * @returns a watcher that awaits stream disposal when its node ends.
 */
export declare function createWatch(remote: ClientRemote): WatchWorkspaceDirectory;
/**
 * One directory listing, bound to a Remote face.
 *
 * The session travels with the call because the endpoint resolves the workspace
 * root from it: the same path means different directories in different sessions.
 * A Remote call does not reject — the result carries the failure.
 */
export type ListWorkspaceDirectory = (sessionId: SessionId, path: string, signal: AbortSignal) => Promise<RemoteResult<DirLevel>>;
/**
 * The slice of the Client Remote face this package calls: the `workspaceFiles`
 * namespace's `list`, exactly as the Host's generated client declares it.
 */
export type WorkspaceFilesListRemote = {
    readonly workspaceFiles: Pick<ClientRemote['workspaceFiles'], 'list'>;
};
/**
 * Bind the listing to one Remote face.
 * @param remote - the Client Remote face carrying the `workspaceFiles` namespace.
 * @returns the listing the tree's face performs.
 */
export declare function createList(remote: WorkspaceFilesListRemote): ListWorkspaceDirectory;
/** The tree's injected business face, as the navigator receives it. */
export interface FilesInjected {
    /**
     * Register a navigator mounted for one tab in one pane, until the returned
     * release. A pane's records — the store's tree, the scroll offset, the file
     * revealed last — last while a navigator is registered there or an open tab,
     * judged by its `tabSignal`, whose latest navigator was there. Once neither
     * holds when the task ends, they are forgotten, the listing closes, and a
     * listing that settles afterwards writes nothing. A tab replaced in place
     * keeps them because its successor registers in the same task; floating or
     * docking a tab never ends them by itself.
     * @param paneId - the pane being drawn.
     * @param tabId - the tab the navigator is drawn for.
     * @param tabSignal - that tab's lifetime.
     * @returns the release.
     */
    readonly attach: (paneId: PaneId, tabId: TabId, tabSignal: AbortSignal) => () => void;
    /**
     * Place this face beside a tab's content for the workspace command, while a
     * navigator that can draw a tree is mounted there, until the returned
     * release; the pane's records do not depend on it.
     * @param tabId - the tab whose content the navigator sits beside.
     * @returns the release.
     */
    readonly placeBeside: (tabId: TabId) => () => void;
    /**
     * List an attached pane's tree at a workspace directory, with its watches,
     * while a navigator draws it. A tree the store keeps at that root is listed
     * again in place, its kept rows still drawn; one at another root, or none, is
     * replaced, resetting the scroll offset and the revealed file. A hold taken
     * in the same task as the last release, as a tab switch's next drawing does,
     * keeps the listing open and lists nothing.
     * @param paneId - the pane being drawn.
     * @param root - absolute path of the workspace root.
     * @returns the release; the listing closes once no hold remains when the current task ends.
     */
    readonly hold: (paneId: PaneId, root: string) => () => void;
    /**
     * Refresh the open directory tree, and run the pane's search again when it
     * has a query, since its results are a snapshot of one search.
     * @param paneId - owning pane.
     */
    readonly refresh: (paneId: PaneId) => void;
    /** Control automatic rereads without closing watches. @param paneId - owning pane. @param enabled - automatic-refresh setting. */
    readonly setAutoRefresh: (paneId: PaneId, enabled: boolean) => void;
    /**
     * Open or collapse one directory, retaining intent during ancestor restoration.
     * @param paneId - the pane being drawn.
     * @param parentPath - the listed parent directory's exact tree key.
     * @param path - absolute directory path.
     * @param expanded - current expansion preferences, including descendants to restore.
     */
    readonly toggle: (paneId: PaneId, parentPath: string, path: string, expanded: readonly string[]) => void;
    /**
     * Record where a pane's tree, or its search results while they are shown,
     * is scrolled to, without rendering anything.
     * @param paneId - the pane being drawn.
     * @param scrollTop - the scroll offset in px.
     */
    readonly scrolled: (paneId: PaneId, scrollTop: number) => void;
    /**
     * Hide a pane's navigator; when its drawing held keyboard focus, the pane's
     * toggle takes it once the commit ends, as on Escape.
     * @param paneId - the pane to hide it in.
     */
    readonly hide: (paneId: PaneId) => void;
    /**
     * Show a pane's navigator and its search bar, and give the field keyboard focus now,
     * or when it is next drawn; a pane drawn but not yet attached shows it once attached.
     * Focusing the always-visible files page leaves the pane's preview-navigator show choice unchanged.
     * A pane with no drawing recorded is left alone.
     * @param paneId - the pane whose search field takes focus.
     */
    readonly focus: (paneId: PaneId) => void;
    /**
     * Record an attached pane's search text. Text with more than whitespace
     * searches the whole workspace with the workspace's current preferences,
     * aborting the pane's running search and dropping its answer; while it holds
     * such text, a change to those preferences searches again. Blank text ends
     * the search, aborting a running one, and the pane draws its tree again.
     * @param paneId - the pane being drawn.
     * @param cwd - the Session's working directory, the search's root; fixed for the Session once known, so a
     *   result always shares the pane's tree root.
     * @param query - the text as typed.
     */
    readonly setQuery: (paneId: PaneId, cwd: string, query: string) => void;
    /**
     * Open or collapse one directory of a pane's search results, leaving the tree's own expansion alone.
     * @param paneId - the pane being drawn.
     * @param path - absolute directory path.
     */
    readonly toggleSearch: (paneId: PaneId, path: string) => void;
    /**
     * Run an attached pane's search again with its current query and the
     * workspace's current preferences, as after a failure; a pane without a
     * query is left alone.
     * @param paneId - the pane being drawn.
     */
    readonly retrySearch: (paneId: PaneId) => void;
    /**
     * Change the search preferences of a workspace for every Session and pane
     * rooted there. Only the fields the patch names change; the others keep
     * their value at the time of the call, not at the time the caller read them.
     * @param cwd - the Session's working directory.
     * @param patch - any of the exclusion rules, the hidden-entry choice, and the `.gitignore` choice.
     */
    readonly setSearchPreferences: (cwd: string, patch: Partial<WorkspaceSearchPreferences>) => void;
    /**
     * Register the drawn search field, or with `null` as gone. Drawing takes
     * keyboard focus that `focus` asked for, or that the previous drawing's
     * field held when it went in the same task.
     * @param paneId - the pane being drawn.
     * @param input - the field, or `null` once it is removed.
     */
    readonly searchDrawn: (paneId: PaneId, input: HTMLElement | null) => void;
    /**
     * Register the tree's scroll container as drawn for one tab, or with `null`
     * as gone. Drawing restores the pane's scroll offset for what it shows — the
     * tree, or the search results, each with its own offset — takes keyboard
     * focus that the previous drawing held when it went, and reveals the tab's
     * file in the tree unless the pane revealed it last. A drawing registers
     * again when it switches between the tree and the results.
     * @param paneId - the pane being drawn.
     * @param address - the tab's content address.
     * @param body - the scroll container, or `null` once it is removed.
     */
    readonly bodyDrawn: (paneId: PaneId, address: string, body: HTMLElement | null) => void;
    /**
     * Register the drawn row of the tab's file, or with `null` as gone; a reveal
     * that completed before the row was drawn scrolls it into view then.
     * @param paneId - the pane being drawn.
     * @param row - the row, or `null` once it is removed or no longer current.
     */
    readonly currentDrawn: (paneId: PaneId, row: HTMLElement | null) => void;
    /**
     * Register a navigator's element, or with `null` as gone. While it is
     * drawn, the face measures the row it is laid out in and, when `closable`,
     * closes the navigator on Escape in that row. When a new width leaves the
     * focused resize edge removed or hidden, the tree takes keyboard focus.
     * When a closable navigator starts filling the row, including its first
     * drawing, focus in the covered content moves to the tree without closing it;
     * so does focus that hiding the covered content left on the document body,
     * unless the reader moved the keyboard or pointer or the window lost focus
     * before the width was reported.
     * @param paneId - the pane being drawn.
     * @param navigator - the navigator element, or `null` once it is removed.
     * @param closable - whether Escape closes this drawing; the `files` page's tree stays shown.
     */
    readonly navigatorDrawn: (paneId: PaneId, navigator: HTMLElement | null, closable: boolean) => void;
    /**
     * Register the drawn show/hide control of a pane's navigator, or with `null` as gone.
     * @param paneId - the pane the control is drawn in.
     * @param toggle - the control, or `null` once it is removed.
     */
    readonly toggleDrawn: (paneId: PaneId, toggle: HTMLElement | null) => void;
    readonly hooks: {
        /** Whether some registered tab type can open an address; a new predicate whenever the tab types change. */
        readonly canOpen: HostObservable<(address: string) => boolean>;
    };
    readonly keyedHooks: {
        /** The measured width of the row a pane's navigator is laid out in, in CSS pixels, keyed by pane id; `undefined` until measured. */
        readonly rowWidth: (paneId: string) => HostObservable<number | undefined>;
        /** A workspace's search preferences, keyed by the Session's working directory; shared by every Session rooted there. */
        readonly searchPreferences: (cwd: string) => HostObservable<WorkspaceSearchPreferences>;
    };
}
/** The slice of the right Sidebar's tab type registry that tells whether an address can be opened. */
export interface TabTypeRegistry {
    /** @returns the registered types; a new array whenever registrations change. */
    entries(): readonly SidebarRightTabDefinition[];
    /** @param address - a resource address. @returns the types that would open it, best first. */
    candidates(address: string): readonly SidebarRightTabDefinition[];
    /** @param listener - called after registrations change. @returns the unsubscribe. */
    subscribe(listener: () => void): () => void;
}
/**
 * Observe whether any registered tab type can open an address.
 * @param registry - the right Sidebar's tab type registry.
 * @returns a source whose snapshot is a predicate, replaced whenever registrations change.
 */
export declare function canOpenOf(registry: TabTypeRegistry): HostObservable<(address: string) => boolean>;
/**
 * Place a face beside a tab's content, where the workspace command finds it,
 * until the returned release.
 * @param tabId - the tab whose content a navigator sits beside.
 * @param face - the face that navigator draws through.
 * @returns the release.
 */
export type PlaceFace = (tabId: TabId, face: FilesInjected) => () => void;
/**
 * Bind the tree's face to one directory listing and one workspace search.
 * @param list - the bound `workspaceFiles.list` call.
 * @param watch - target-scoped directory observation.
 * @param canOpen - whether some tab type can open an address.
 * @param navigationOf - the attached tab's navigation; subscriptions end with this plugin or their disposer.
 * @param search - the workspace search.
 * @param preferences - every workspace's search preferences, shared by every Session.
 * @returns the face factory: session, its store instance, and where to place the face beside tabs in, face out.
 */
export declare function filesFace(list: ListWorkspaceDirectory, watch: WatchWorkspaceDirectory, canOpen: HostObservable<(address: string) => boolean>, navigationOf: (sessionId: SessionId, tabId: TabId) => HostObservable<SidebarRightTabNavigation>, search: SearchWorkspace, preferences: WorkspaceSearchPreferenceBook): (sessionId: SessionId, store: Pick<FilesStoreInstance, 'actions' | 'getSnapshot' | 'subscribe'>, place: PlaceFace) => FilesInjected;
//# sourceMappingURL=face.d.ts.map