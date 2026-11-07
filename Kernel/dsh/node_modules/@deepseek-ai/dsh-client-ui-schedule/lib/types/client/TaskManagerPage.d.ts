import type { HostObservable, InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ScheduleCatalogEntry, ScheduleCreateRequest } from '@deepseek-ai/dsh-schedule/client';
import type { TaskCreateOutcome } from './TaskDetail.tsx';
import type { TaskEditorStoreProps } from './editor-store.ts';
import type { CatalogSnapshot } from './catalog-source.ts';
import { type TaskDetailInjected } from './TaskDetail.tsx';
/** Injected catalog and task actions for the management page. */
export interface TaskManagerInjected extends TaskDetailInjected {
    /** Host task catalog with its query and deletion state. */
    readonly hooks: {
        readonly catalog: HostObservable<CatalogSnapshot<ScheduleCatalogEntry>>;
    };
    /**
     * Start a new Session, where a reminder is created by asking the model to
     * schedule it, using the existing new-conversation action.
     */
    readonly onNewTask: () => void;
    /**
     * Remove every ended Host task, after this page has confirmed the action.
     *
     * The call requests no task ids, so the Host's ended set is the one removed;
     * the page's rows are only a snapshot that a delivery can have moved. It
     * resolves once the settled outcome has been reported through the app-wide
     * notice, which is the only place the result is stated.
     * @returns resolution after the outcome is reported.
     */
    readonly onClearEnded: () => Promise<void>;
    /** Create a task directly through the Host, without a model request. */
    readonly onCreateTask: (request: ScheduleCreateRequest) => Promise<TaskCreateOutcome>;
}
/** Root-scoped task catalog props derived from the framework and injected actions. */
export type TaskManagerPageProps = PropsRuntime<'main'> & InjectFace<TaskManagerInjected> & TaskEditorStoreProps & PropsLocale<'schedule.manager'>;
/**
 * Render retained tasks with manual creation, inline editing, and authoritative deletion.
 * @param props - framework catalog snapshot, localized copy, and action callbacks.
 * @returns the searchable task list beside the selected task's detail.
 */
export declare function TaskManagerPage(props: TaskManagerPageProps): import("react").JSX.Element;
//# sourceMappingURL=TaskManagerPage.d.ts.map