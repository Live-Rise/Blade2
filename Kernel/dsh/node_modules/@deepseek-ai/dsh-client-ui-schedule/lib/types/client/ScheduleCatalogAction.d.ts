import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ScheduleId, ScheduleCatalogEntry } from '@deepseek-ai/dsh-schedule/client';
import type { CatalogInjected } from './catalog-source.ts';
import { NS } from './locales.ts';
/** Full props for the Session-header Schedule catalog action. */
export type ScheduleCatalogActionProps = PropsRuntime<'conversation.session.header.utilities'> & PropsLocale<typeof NS> & InjectFace<CatalogInjected<ScheduleCatalogEntry> & {
    /**
     * Show one task's detail in the right Sidebar for this entry's Session.
     * @param id - Task chosen by the Session entry.
     */
    readonly openTaskDetail: (id: ScheduleId) => void;
}>;
/**
 * Current-Session reminder catalog with durable deletion, rendered only while
 * the current Peer may list this Session's reminders.
 * @param props - the Session seats, the injected catalog and verbs, and localized copy.
 * @returns the reminder catalog action, or nothing without access.
 */
export declare function ScheduleCatalogAction(props: ScheduleCatalogActionProps): import("react").JSX.Element | null;
//# sourceMappingURL=ScheduleCatalogAction.d.ts.map