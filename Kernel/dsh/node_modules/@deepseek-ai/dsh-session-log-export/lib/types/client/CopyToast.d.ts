import type { ReactNode } from 'react';
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionMarkdownCopyState } from './copy-controller.ts';
import { NS } from './locales.ts';
/** Copy state and dismissal bound into the root overlay entry. */
export interface SessionMarkdownCopyToastInjected {
    hooks: {
        sessionMarkdownCopy: ObservableSnapshot<SessionMarkdownCopyState>;
    };
    dismiss: () => void;
}
/** Props of the root overlay copy notice. */
export type SessionMarkdownCopyToastProps = PropsRuntime<'shell.overlay'> & PropsLocale<typeof NS> & InjectFace<SessionMarkdownCopyToastInjected>;
/**
 * Show the settled copy outcome outside the Session Header and menu lifetime.
 * @param props - copy notice hook, dismissal action, and localized copy.
 * @returns the success or failure toast, or null while no notice shows.
 */
export declare function SessionMarkdownCopyToast({ useSessionMarkdownCopy, dismiss, t, }: SessionMarkdownCopyToastProps): ReactNode;
//# sourceMappingURL=CopyToast.d.ts.map