/** Original-Session link availability from the current public Session and Workspace feeds. */
import type { SessionListState } from '@deepseek-ai/dsh-api-session-controller/client';
import type { WorkspaceSnapshot } from '@deepseek-ai/dsh-api-workspace-controller/client';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
/** Navigation state shown beside the retained task's original Session id. */
export type SessionLinkState = 'available' | 'loading' | 'archived' | 'unavailable';
/**
 * Check Host-list membership and archive state without activating or restoring anything.
 *
 * `SessionListState.byId` also carries local fallback rows for live Client
 * generations, so membership comes from `ids`, the Host-list projection: a
 * Session the Host list dropped reports unavailable even while a local row for
 * it survives.
 * @param id - Original Session bound to the task.
 * @param sessions - Current Session list projection.
 * @param workspaces - Current Workspace and archive projection.
 * @returns availability or the reason navigation is disabled.
 */
export declare function sessionLinkState(id: SessionId, sessions: SessionListState, workspaces: WorkspaceSnapshot): SessionLinkState;
//# sourceMappingURL=session-link.d.ts.map