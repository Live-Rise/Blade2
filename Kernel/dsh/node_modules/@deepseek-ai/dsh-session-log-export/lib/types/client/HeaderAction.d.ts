import type { ReactNode } from 'react';
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { InjectFace } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import { type SessionLogDownloadDialogProps } from './Dialog.tsx';
import type { SessionLogDownloadDialogInjected } from './Dialog.tsx';
import type { SessionMarkdownCopyState } from './copy-controller.ts';
/** Markdown copy state and action bound into the Session Header contribution. */
export interface SessionMarkdownCopyHeaderInjected {
    hooks: {
        sessionMarkdownCopy: ObservableSnapshot<SessionMarkdownCopyState>;
    };
    /**
     * Copy one complete Session transcript to the clipboard.
     * @param sessionId - Session captured by the menu click.
     */
    copyMarkdown: (sessionId: SessionId) => Promise<void>;
}
/** Session download controls plus Markdown copy and observable feedback availability. */
export interface SessionLogDownloadHeaderInjected extends SessionLogDownloadDialogInjected, SessionMarkdownCopyHeaderInjected {
    hooks: SessionLogDownloadDialogInjected['hooks'] & SessionMarkdownCopyHeaderInjected['hooks'] & {
        feedbackAvailable: ObservableSnapshot<boolean>;
    };
    /**
     * Open the existing Session feedback draft without recording feedback; no-op after the feedback plugin unloads.
     * @param sessionId - Session whose feedback form to open.
     */
    openFeedback: (sessionId: SessionId) => void;
}
/** Session download props plus the optional feedback action. */
export type SessionLogDownloadHeaderProps = SessionLogDownloadDialogProps & InjectFace<SessionLogDownloadHeaderInjected>;
/**
 * Render the Session Header menu with Markdown copy, download, and optional
 * feedback actions.
 * @param props - Session runtime, download and copy state, and localized copy.
 * @returns the persistent Header action and Session-scoped dialog.
 */
export declare function SessionLogDownloadHeaderAction(props: SessionLogDownloadHeaderProps): ReactNode;
//# sourceMappingURL=HeaderAction.d.ts.map