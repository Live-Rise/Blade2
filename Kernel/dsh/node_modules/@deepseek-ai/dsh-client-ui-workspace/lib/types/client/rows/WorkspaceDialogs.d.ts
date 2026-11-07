import type { WorkspaceView } from '@deepseek-ai/dsh-api-workspace-controller/client';
import type { WorkspaceActionRequest, WorkspaceBrowserProps } from '../contract/slots.ts';
type Props = Pick<WorkspaceBrowserProps, 'renameWorkspace' | 'deleteWorkspace' | 'settleWorkspaceAction' | 't'> & {
    request: WorkspaceActionRequest;
    workspaces: readonly WorkspaceView[];
};
/** @param props - captured request, current Workspace list, and existing mutations. @returns the active dialog. */
export declare function WorkspaceDialogs({ request, workspaces, renameWorkspace, deleteWorkspace, settleWorkspaceAction, t }: Props): import("react").JSX.Element;
export {};
//# sourceMappingURL=WorkspaceDialogs.d.ts.map