import type { ReactNode } from 'react';
import type { FactoryComponentPropsOf } from '@deepseek-ai/dsh-client-ui-slots';
import type { ContextMenuTriggerProps } from '@deepseek-ai/dsh-client-ui-context-menu/client';
/** What a row is: a directory toggles, a file opens, and anything else is listed without either. */
export type TreeEntryKind = 'file' | 'directory' | 'other';
/** The standard facts the tree reads about an entry; every field is optional. */
export interface TreeEntryMeta {
    /** What the entry is. Without it, an entry with other entries below its path is a directory, and any other is a file. */
    readonly kind?: TreeEntryKind;
    /** The row is drawn subdued; the entry and those below it are still listed. */
    readonly hidden?: boolean;
    /** Size in bytes; absent means unknown. The tree does not draw it. */
    readonly size?: number;
}
/**
 * One tree entry. `path` identifies it and places it: its parent is the part
 * before the last `/`, or the tree's root when it has no `/`. `name` is what
 * the row shows and need not be the end of `path`.
 */
export interface TreeEntry {
    /** The entry's key; only `/` separates levels. */
    readonly path: string;
    /** The name drawn on the row. */
    readonly name: string;
    /** The standard facts the tree reads. */
    readonly meta?: TreeEntryMeta;
}
/** What the caller knows about one directory's listing besides its entries; every field is optional. */
export interface TreeDirectoryState {
    /** The listing is not known yet; the directory shows a loading line instead of its entries. */
    readonly loading?: true;
    /** Why the directory could not be listed, already localized; the entries known so far still show. */
    readonly failure?: string;
    /** Entries are missing from the listing. */
    readonly truncated?: true;
    /**
     * The caller left entries out, which changes what the directory says while
     * none is drawn: `hidden` says that its hidden entries are not shown, and
     * `unmatched`, for entries that are a selection such as search matches, says
     * nothing instead of that it is empty.
     */
    readonly omitted?: 'hidden' | 'unmatched';
}
/** The `sidebar.files.tree` Factory's input: data and callbacks only. */
export interface FileTreeInput {
    /** The root's key; the root itself has no row and its entries are always shown. */
    readonly root: string;
    /**
     * Every entry the caller knows, in any order, each `path` once. Entries
     * whose `path` equals `root` are ignored, including when both are empty.
     * A parent key below the root without an entry of its own gets a directory
     * entry named after the key's last segment; the empty
     * key is never added. An entry under neither the root nor an expanded
     * directory is not drawn.
     */
    readonly entries: readonly TreeEntry[];
    /** Keys of the expanded directories. */
    readonly expanded: readonly string[];
    /** Listing state by directory key, root included; a directory absent here is listed and complete. */
    readonly directories?: Readonly<Record<string, TreeDirectoryState>> | undefined;
    /** Text whose occurrences inside each name are highlighted, ignoring case; nothing is highlighted without it. */
    readonly highlight?: string | undefined;
    /** Key of the file to mark as current, when it is drawn. */
    readonly current?: string | undefined;
    /** Receives the current file's main button while drawn. @param row - the button, or `null` once it goes. */
    readonly currentRef?: ((row: HTMLElement | null) => void) | undefined;
    /**
     * A directory row was pressed.
     * @param entry - the caller's directory entry, or the one the tree added.
     * @param expanded - whether the press asks to expand it.
     */
    readonly onToggle: (entry: TreeEntry, expanded: boolean) => void;
    /**
     * An openable file row was pressed.
     * @param entry - the caller's entry.
     */
    readonly onOpen: (entry: TreeEntry) => void;
    /**
     * Whether a file opens; a file it refuses is dimmed and titled with the localized no-viewer message. Every file opens without it.
     * @param entry - the caller's entry.
     * @returns `false` to refuse it.
     */
    readonly canOpen?: ((entry: TreeEntry) => boolean) | undefined;
    /**
     * Context menu handlers for a directory row or an openable file row's main button.
     * @param entry - the caller's entry, or the directory the tree added.
     * @param kind - the kind the tree resolved for the row.
     * @returns the handlers spread onto the row's button.
     */
    readonly rowTriggerProps?: ((entry: TreeEntry, kind: 'file' | 'directory') => ContextMenuTriggerProps) | undefined;
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface SlotFactoryMap {
        /** A controlled file tree over caller-supplied entries; the caller decides what a click does. */
        'sidebar.files.tree': {
            scope: 'root';
            props: FileTreeInput;
            locale: 'sidebarFiles';
            slots: {
                /** Independent content beside each row's main action. */
                accessory: {
                    scope: 'root';
                    props: {
                        readonly entry: TreeEntry;
                    };
                };
            };
        };
    }
}
/**
 * The tree from its root.
 * @param props - the caller's entries, expansion, states, callbacks, and framework seats.
 * @returns the root level's list.
 */
declare function FileTreeView({ t, useFactorySlot, ...input }: FactoryComponentPropsOf<'sidebar.files.tree'>): ReactNode;
/**
 * The file tree. It redraws when a prop or the occurrence's local selection
 * changes, so a caller keeps its inputs and local selection stable between renders.
 */
export declare const FileTree: import("react").MemoExoticComponent<typeof FileTreeView>;
export {};
//# sourceMappingURL=FileTree.d.ts.map