/** Root overlay showing one Session export at a time in initiation order. */
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionLogDownloadState } from './controller.ts';
import { NS } from './locales.ts';
/** Download state and dismissal bound into the root overlay entry. */
export interface SessionLogDownloadDialogInjected {
    hooks: {
        sessionLogDownload: ObservableSnapshot<SessionLogDownloadState>;
    };
    dismiss: (sessionId: SessionId) => void;
}
/** Root runtime, localized copy, and injected download state and actions. */
export type SessionLogDownloadDialogProps = PropsRuntime<'shell.overlay'> & PropsLocale<typeof NS> & InjectFace<SessionLogDownloadDialogInjected>;
/**
 * Show the earliest open export independently of the displayed conversation.
 * @param props - root runtime, bound controller state, dismissal, and localized copy.
 * @returns one modal, or null when all exports are dismissed.
 */
export declare function SessionLogDownloadDialog({ useSessionLogDownload, dismiss, t, }: SessionLogDownloadDialogProps): import("react").JSX.Element | null;
//# sourceMappingURL=Dialog.d.ts.map