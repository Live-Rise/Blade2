/** One pending request to open a Session's share dialog after switching to it. */
/**
 * Bind the workspace navigation one manager click uses.
 * @param open - Workspace Session opener, or undefined when this page has none.
 */
export declare function setManagedShareOpener(open: ((sessionId: string) => void) | undefined): void;
/**
 * Switch to one Session before its header action opens the share dialog.
 * @param sessionId - Session the manager selected.
 */
export declare function openManagedSession(sessionId: string): void;
/**
 * Remember the Session whose share dialog should open once its header action mounts.
 * @param sessionId - Session the manager selected.
 */
export declare function rememberManagedShare(sessionId: string): void;
/**
 * Consume a pending request for one Session.
 * @param sessionId - Session whose header action is mounting.
 * @returns True when this Session's share dialog should open.
 */
export declare function takeManagedShare(sessionId: string): boolean;
/** Drop a request that navigation could not complete. */
export declare function forgetManagedShare(): void;
//# sourceMappingURL=share-navigation.d.ts.map