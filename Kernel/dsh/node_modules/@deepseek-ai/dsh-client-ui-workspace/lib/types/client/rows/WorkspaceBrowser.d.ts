import type { WorkspaceBrowserProps } from '../contract/slots.ts';
/**
 * Render the browsing region.
 * @param props - composed slot props (shell owner share + store + injected actions).
 * @returns the region element tree.
 */
export declare function WorkspaceBrowser({ wide, usePanelInfo, expandSidebar, useSectionScroller, useSessions, useSessionStatus, useSessionAccess, useWorkspaceAccess, useWorkspaces, useStore, actions, startSession, open, requestSessionRename, notifyArchivedNotOpenable, renameWorkspace, useWorkspaceActionRequest, settleWorkspaceAction, deleteWorkspace, insertWorkspaceBefore, unarchiveSession, createWorkspace, searchSessions, searchResultLimit, useDirectoryFlow, useHostInfo, useShortcuts, useWorkspaceShortcuts, requestSearch, requestAddWorkspace, closeAddWorkspace, setDirectoryBusy, renderSlot, renderFactorySlot, useContextMenuTarget, useContextMenu, t, }: WorkspaceBrowserProps): import("react").JSX.Element;
//# sourceMappingURL=WorkspaceBrowser.d.ts.map