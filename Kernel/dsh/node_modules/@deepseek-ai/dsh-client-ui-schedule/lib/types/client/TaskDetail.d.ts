import type { ReactNode } from 'react';
import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ScheduleCatalogEntry, ScheduleCreateRequest, ScheduleEditRequest, ScheduleId } from '@deepseek-ai/dsh-schedule/client';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import { type DeliveryHistoryInjected } from './DeliveryHistory.tsx';
import type { CatalogDeleteOutcome, CatalogSnapshot } from './catalog-source.ts';
import type { TaskEditorStoreProps } from './editor-store.ts';
/** Localized reason the Host refused an edit, or reason the wiring could not confirm it. */
export type HostEditFailure = 'host.saveError' | 'host.conflict' | 'host.notFuture' | 'host.invalidRule' | 'detail.missing';
/** Localized reason the Host refused a run, or reason the wiring could not confirm it. */
export type HostRunFailure = 'host.runError' | 'host.executionPending' | 'detail.missing';
/** Creation keeps a failed draft and never creates or activates its destination Session. */
export type TaskCreateOutcome = {
    readonly created: ScheduleCatalogEntry;
} | {
    readonly failed: 'host.createError' | 'host.notFuture' | 'host.invalidRule';
};
/** Target-based mutations supplied by the owning plugin's Remote wiring. */
export interface HostTaskActions {
    /**
     * Apply one edit and refresh the catalog after the Host answers.
     * @param request - task, change, and the `updatedAt` value that the change started from.
     * @returns `saved`, or the localized reason the edit did not take effect.
     */
    readonly edit: (request: ScheduleEditRequest) => Promise<'saved' | {
        readonly failed: HostEditFailure;
    }>;
    /**
     * Queue one run now and refresh the catalog after the Host answers.
     * @param id - task to run.
     * @returns `queued`, or the localized reason the run did not start.
     */
    readonly run: (id: ScheduleId) => Promise<'queued' | {
        readonly failed: HostRunFailure;
    }>;
    /** Read the configured minimum new-conversation interval; undefined means the read failed. */
    readonly loadNewSessionInterval: () => Promise<number | undefined>;
}
/** The two views of one task's detail: its rule, or its saved delivery records. */
export type TaskDetailTab = 'rule' | 'records';
/** Deletion, Host edit and run, saved-history, and linked-Session actions one task's detail calls. */
export interface TaskDetailInjected extends DeliveryHistoryInjected {
    /** Host-owned edit, run, and new-Session interval actions. */
    readonly hostActions: HostTaskActions;
    /**
     * Delete one task.
     * @param id - Task shown in this detail.
     * @returns The deletion's outcome after the Remote acknowledgement and
     * authoritative refresh; the wiring reports it as the app-wide toast.
     */
    readonly onDelete: (id: ScheduleId) => Promise<CatalogDeleteOutcome>;
    /**
     * Reload the catalog after a query failure or an acknowledged edit,
     * and from a surface that needs a read newer than its own record.
     * @param since - request ordinal the caller last observed, 0 to share any read in flight.
     * @returns Resolution after publishing the read result; failures remain in catalog state.
     */
    readonly onRetry: (since?: number) => Promise<void>;
    /**
     * Open an available, unarchived linked conversation after rechecking current metadata.
     * @param id - Session linked to the shown task.
     */
    readonly onOpenSession: (id: SessionId) => void;
}
/** One task's detail, as the Tasks page and the task tab both compose it. */
type TaskDetailCommonProps = TaskDetailInjected & TaskEditorStoreProps & PropsLocale<'schedule.manager'> & Pick<PropsRuntime<'main'>, 'useSessions' | 'useWorkspaces'> & {
    /** Element id shared by the detail panels and any control that points at them. */
    readonly id: string;
    /** Catalog query state; loading disables saved-task actions. */
    readonly status: 'loading' | 'ready' | 'error';
    /** Tasks with a deletion in flight. */
    readonly deleting: readonly ScheduleId[];
    /**
     * Retain a task while a confirmed deletion and its catalog refresh settle.
     */
    readonly onRetainTask: (task: ScheduleCatalogEntry | null) => void;
    /** Active detail view; the owner resets it to Rules when it selects another task. */
    readonly tab: TaskDetailTab;
    /** Select the active detail view. */
    readonly onTabChange: (tab: TaskDetailTab) => void;
    /** Task whose deletion confirmation the owner asked for, or null when none is open. */
    readonly confirmId: ScheduleId | null;
    /** Ask for one task's deletion confirmation, or dismiss it with null. */
    readonly onConfirm: (id: ScheduleId | null) => void;
    /**
     * Leave the shown task once the Host applied its deletion and the catalog refresh after it
     * settled: the Tasks page clears its selection and the session task tab closes.
     */
    readonly onDeleted: () => void;
    /** Close this detail; a view with no list to return to omits the control. */
    readonly onClose?: () => void;
};
/** New and existing tasks use the same editable page; only submission and saved-task actions differ. */
export type TaskDetailProps = TaskDetailCommonProps & ({
    readonly task: ScheduleCatalogEntry;
    readonly authoritative: boolean;
    readonly onCreateTask?: never;
    readonly onCreated?: never;
} | {
    readonly task?: undefined;
    readonly authoritative?: never;
    readonly onCreateTask: (request: ScheduleCreateRequest) => Promise<TaskCreateOutcome>;
    readonly onCreated: (task: ScheduleCatalogEntry) => void;
});
/** One task detail's selected tab, confirmation, and retained catalog row. */
export interface TaskDetailController {
    /** Catalog row the shown id resolves to, or undefined when the read has no such row. */
    readonly record: ScheduleCatalogEntry | undefined;
    /** Task to show: the catalog row, an unsaved draft's baseline, or a confirmed deletion's retained row. */
    readonly task: ScheduleCatalogEntry | undefined;
    /** Props the shared detail element takes, without the task it shows. */
    readonly props: Omit<TaskDetailCommonProps, 'onClose' | 'onDeleted'>;
}
/**
 * Own selected-task state shared by the Tasks page and task tabs.
 *
 * The shown task comes from the catalog, an unsaved form's CAS baseline, or a
 * confirmed deletion. The view and confirmation reset when selection changes;
 * the entry's store retains unsaved fields by task identity.
 * @param injected - detail actions, localized copy, and framework readers.
 * @param catalog - the owner's authoritative task catalog snapshot.
 * @param taskId - task the owner selected, or undefined when it selected none.
 * @returns the shown task and catalog row, its element id, its confirmation, and the detail props.
 */
export declare function useTaskDetail(injected: TaskDetailInjected & TaskEditorStoreProps & PropsLocale<'schedule.manager'> & Pick<PropsRuntime<'main'>, 'useSessions' | 'useWorkspaces'>, catalog: CatalogSnapshot<ScheduleCatalogEntry>, taskId: ScheduleId | undefined): TaskDetailController;
/**
 * Render one new or existing task's editable rule or saved deliveries.
 * @param props - Task, catalog state, detail view, confirmation, locale, and actions.
 * @returns The detail region and its deletion confirmation dialog.
 */
export declare function TaskDetail(props: TaskDetailProps): ReactNode;
export {};
//# sourceMappingURL=TaskDetail.d.ts.map