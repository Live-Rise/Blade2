import type { PropsRenderSlots } from '@deepseek-ai/dsh-client-ui-slots';
import type { ShortcutCatalogEntry } from '@deepseek-ai/dsh-client-shortcuts/client';
import type { WorkspaceBrowserProps } from '../contract/slots.ts';
import type { GroupNode, SearchResultNode, SessionNode } from '../tree.ts';
/** The standard locale seat, prop-passed from the browser root. */
type RowTranslate = WorkspaceBrowserProps['t'];
/**
 * Row drag wiring supplied by the tree owner. `drop` reports the half of the
 * row where the pointer released so the owner can resolve an insert anchor.
 */
export interface RowDragProps {
    /** Start dragging this row. */
    start: () => void;
    /** A compatible row drag is in flight. */
    active: boolean;
    /** Current marker on this row: insert line above, below, or none. */
    marker: 'before' | 'after' | null;
    /** Report the hovered half while a compatible drag passes over this row. */
    hover: (half: 'before' | 'after') => void;
    drop: (half: 'before' | 'after') => void;
    end: () => void;
}
/** Drag lifecycle owned by a workspace row; its enclosing group owns hit testing. */
interface WorkspaceRowDragProps {
    start: () => void;
    end: () => void;
}
/** Owner props of one Workspace header row; the access Hooks are the browser root's own standard seats. */
type ProjectRowItemProps = {
    group: GroupNode;
    newShortcut?: ShortcutCatalogEntry | undefined;
    containsCurrentDescendant?: boolean;
    onToggle: () => void;
    onCreate: () => void;
    /** Present only for real Workspace rows in the grouped view. */
    drag?: WorkspaceRowDragProps | undefined;
    /** Host account home; POSIX home-rooted hover paths display as `~`. */
    home?: string | undefined;
    t: RowTranslate;
} & Pick<WorkspaceBrowserProps, 'useRemoteAccess' | 'useWorkspaceAccess' | 'openWorkspaceMenu' | 'useContextMenuTarget' | 'useContextMenu'>;
/**
 * Project (workspace) header row: folder + title;
 * hover reveals the chevron and create button, and dwelling on a real
 * Workspace shows its hover card (the ungrouped bucket has none).
 * `containsCurrent` arrives on the node (derivation fact, no renderer scan).
 * Create and drag follow the Workspace access snapshot. Menu providers own
 * their access checks; the ungrouped bucket has no Workspace menu.
 * @param props.group - derived group node.
 * @param props.containsCurrentDescendant - highlight an ancestor even when its subtree is collapsed.
 * @param props.onToggle - expand/collapse the group.
 * @param props.onCreate - start a frontend Session inside this Workspace.
 * @param props.drag - optional workspace-row drag wiring.
 * @param props.home - host account home for POSIX hover-path abbreviation.
 * @param props.t - the browser root's locale seat.
 * @returns the row element.
 */
export declare function ProjectRowItem(props: ProjectRowItemProps): import("react").JSX.Element;
/**
 * One flat search result: title, Workspace context, and optional content
 * excerpt. Search navigation opens the session only; it does not address an
 * event inside the conversation. Archived rows carry a hover unarchive
 * button, because search is where the filter surfaces them for recovery.
 * @param props.result - merged local/content search row.
 * @param props.currentId - selected session id.
 * @param props.onOpen - open the selected session.
 * @param props.onUnarchive - unarchive an archived result row.
 * @param props.useSessionAccess - the browser root's Session access Hook; the restore control
 * appears only while the snapshot allows `workspace/unarchiveSession` for that Session.
 * @param props.t - Workspace-browser translation seat.
 * @returns the result row.
 */
export declare function SearchResultItem({ result, currentId, onOpen, onUnarchive, useSessionAccess, t, }: {
    result: SearchResultNode;
    currentId: string | undefined;
    onOpen: (id: SearchResultNode['id']) => void;
    onUnarchive: (id: SearchResultNode['id']) => void;
    t: RowTranslate;
} & Pick<WorkspaceBrowserProps, 'useSessionAccess'>): import("react").JSX.Element;
/**
 * One top-level 32px session row: leading 16px cell (status dot, or the
 * leading seat while the row's primary state is idle), title, relative time or
 * compact pending label, and the row actions menu. A row that owns a state dot
 * keeps that cell and renders no seat, so an ambient automation mark never
 * appears beside the row's own state dot. An archived row keeps the cell blank:
 * neither marker renders there, and its live status stays on the hover card.
 * The row accepts Tab focus and opens with unmodified Enter or Space; descendant controls
 * handle their own activation without opening the row.
 * @param props.node - derived session node.
 * @param props.currentId - selected session id (row highlight).
 * @param props.now - epoch ms for relative-time formatting.
 * @param props.onOpen - open a session by id.
 * @param props.onRenameRequest - open the rename dialog from a title double-click (id + current title);
 * called only after a single press on the title since the window last lost focus.
 * @param props.renderSlot - child-seat renderer for the row's action lists
 * (`sidebar.workspaces.session.menu.item` / `sidebar.workspaces.session.row.action`),
 * its leading and trailing decorations, and its hover-card section.
 * @param props.onReveal - scroll this row into view after search navigation, then acknowledge it.
 * @param props.drag - optional row-drag target wiring; blank rows, and rows whose access excludes
 * `workspace/insertSessionBefore`, cannot start a drag.
 * @param props.home - host account home for POSIX hover-path abbreviation.
 * @param props.useSessionAccess - the browser root's Session access Hook; the title double-click
 * renames only while it allows `session/rename`.
 * @param props.t - the browser root's locale seat.
 * @returns the session row.
 */
export declare function SessionNodeItem({ node, currentId, now, onOpen, onRenameRequest, renderSlot, openSessionMenu, useContextMenuTarget, useContextMenu, onReveal, drag, home, useSessionAccess, t, }: {
    useContextMenu: WorkspaceBrowserProps['useContextMenu'];
    openSessionMenu: WorkspaceBrowserProps['openSessionMenu'];
    useContextMenuTarget: WorkspaceBrowserProps['useContextMenuTarget'];
    node: SessionNode;
    currentId: string | undefined;
    now: number;
    onOpen: (id: SessionNode['id']) => void;
    /** Open the rename dialog from a title double-click (id + current title); see the function JSDoc for the press precondition. */
    onRenameRequest: (id: SessionNode['id'], currentTitle: string) => void;
    /** Scroll this row into view after search navigation, then acknowledge it. */
    onReveal?: (() => void) | undefined;
    /** Present on reorderable-list rows so every row can remain a drop target. */
    drag?: RowDragProps | undefined;
    /** Host account home; POSIX home-rooted hover paths display as `~`. */
    home?: string | undefined;
    t: RowTranslate;
} & Pick<WorkspaceBrowserProps, 'useSessionAccess'> & PropsRenderSlots<'sidebar.workspaces.session.row.action' | 'sidebar.session.row.trailing' | 'sidebar.session.row.leading' | 'sidebar.session.row.hover'>): import("react").JSX.Element;
export {};
//# sourceMappingURL=Rows.d.ts.map