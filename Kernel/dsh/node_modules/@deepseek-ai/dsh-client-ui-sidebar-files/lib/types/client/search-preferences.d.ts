/**
 * The search preferences of every workspace, kept in browser storage and
 * shared by every Session and pane whose workspace root is the same.
 *
 * The storage value maps a normalized workspace root to the fields the reader
 * changed there. A field equal to its default is left out of the entry, a root
 * the reader never changed has no entry, and a change back to the defaults
 * removes its entry, so the defaults of a later build reach every field the
 * reader left alone. Storage
 * holds whatever an earlier build or the reader wrote: each entry is read
 * through {@link readSearchPreferences} rather than trusted.
 */
import type { SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { HostObservable } from '@deepseek-ai/dsh-client-ui-slots';
import { type WorkspaceSearchPreferences } from './search-rules.ts';
/**
 * The localStorage key the preferences of every workspace are stored under.
 *
 * Each entry holds whichever of `rules`, `showHidden`, and `respectGitignore`
 * differ from their defaults. Every field is optional on read:
 * {@link readSearchPreferences} gives a missing or mistyped one its current
 * default and reads the others as usual. A change that an earlier build would
 * misread takes a new key.
 */
export declare const SEARCH_PREFERENCES_KEY = "dsh.sidebar-files.workspace-search.v1";
/**
 * The storage key of one workspace root: the root as the Host reports it, with
 * trailing `/` and `\` removed except the one that makes a filesystem root
 * (`/`, `C:\`, `C:/`). Case and separators are otherwise kept, so two
 * spellings of one directory keep separate preferences.
 * @param cwd - the Session's working directory.
 * @returns the key its preferences are stored under.
 */
export declare function workspaceKey(cwd: string): string;
/** The search preferences of every workspace root. */
export interface WorkspaceSearchPreferenceBook {
    /**
     * Observe one root's preferences. The same root, by its {@link workspaceKey},
     * gets the same source, and its snapshot keeps its reference until that
     * root's preferences change.
     * @param cwd - the Session's working directory.
     * @returns the source.
     */
    readonly source: (cwd: string) => HostObservable<WorkspaceSearchPreferences>;
    /**
     * Change the reader's choice for one root: the fields the patch names
     * replace those of the root's current preferences, read at the call; the
     * entry keeps only the fields that differ from the defaults, and is removed
     * when none does.
     * @param cwd - the Session's working directory.
     * @param patch - any of the exclusion rules, the hidden-entry choice, and the `.gitignore` choice.
     */
    readonly update: (cwd: string, patch: Partial<WorkspaceSearchPreferences>) => void;
}
/**
 * Keep the preferences of every workspace root in one persisted store.
 * @param store - the persisted store; its rehydrated value may be anything JSON can hold.
 * @returns the preferences by workspace root.
 */
export declare function createSearchPreferenceBook(store: SnapshotStore<unknown>): WorkspaceSearchPreferenceBook;
//# sourceMappingURL=search-preferences.d.ts.map