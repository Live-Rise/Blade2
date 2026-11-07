/** Display the Host's effective admission interval before selecting per-run conversations. */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { HostTaskActions } from './TaskDetail.tsx';
/** Read the configured interval through an injected action; failed reads stay visible and retryable.
 * @param props - The failure-contained read action and task-manager translations.
 * @returns The configured interval, a loading indicator, or a retryable failure.
 */
export declare function NewSessionLimit({ load, t }: PropsLocale<'schedule.manager'> & {
    readonly load: HostTaskActions['loadNewSessionInterval'];
}): ReactNode;
//# sourceMappingURL=NewSessionLimit.d.ts.map