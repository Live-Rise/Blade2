/** App-wide notice for forge panel actions, rendered outside the panel so it outlives it. */
import type { ReactNode } from 'react';
import type { InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { NoticeSource } from './source.ts';
/** Full props of the `shell.overlay` entry. */
export type ActionToastProps = PropsLocale<typeof NS> & InjectFace<Omit<NoticeSource, 'report'>>;
/**
 * Render the current notice: a success notice with its undo action, or a failure warning.
 * @param props - notice hook, dismissal, and copy.
 * @returns the toast, or nothing while no notice shows.
 */
export declare function ActionToast({ useNotice, dismiss, t }: ActionToastProps): ReactNode;
//# sourceMappingURL=ActionToast.d.ts.map