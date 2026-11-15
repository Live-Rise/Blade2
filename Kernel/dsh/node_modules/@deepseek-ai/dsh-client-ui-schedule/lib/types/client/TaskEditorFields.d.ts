/** Fields shared by manual creation and editing inside the existing Rules view. */
import type { ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { WorkspaceSnapshot } from '@deepseek-ai/dsh-api-workspace-controller/client';
import type { ScheduleCatalogEntry, SchedulePatch, ScheduleTargetRequest } from '@deepseek-ai/dsh-schedule/client';
import type { TimingDraft } from './editor-timing.ts';
import type { TaskManagerKey } from './task-manager-locales.ts';
/** Local form values; only Save writes a task. */
export interface TaskEditorDraft extends TimingDraft {
    readonly title: string;
    readonly prompt: string;
    readonly target: 'session' | 'new_once' | 'new';
    readonly sessionId: string;
    readonly cwd: string;
}
/** Registered Workspaces offered as the directory of a created Session. */
interface ProjectChoices {
    readonly items: readonly {
        readonly value: string;
        readonly label: string;
    }[];
    /** Directory a new destination starts with: the default Workspace, else the first listed one, else empty. */
    readonly defaultCwd: string;
}
/** Label Workspaces as the Workspace list does and select the directory a new destination starts with. */
export declare function projectChoices(workspaces: WorkspaceSnapshot['items'], defaultName: string): ProjectChoices;
/** Initialize a new task without creating or activating a Session. */
export declare function newTaskDraft(): TaskEditorDraft;
/** Seed an edit without normalizing unsupported stored rules. */
export declare function taskDraft(task: ScheduleCatalogEntry): TaskEditorDraft;
/**
 * Validate the form while retaining untouched stored timing.
 * @param draft - Current form values.
 * @param changes - Existing task patch; omitted validates a new timing as well.
 * @returns First correction key, `incomplete` for missing required fields, or undefined when valid.
 */
export declare function taskDraftIssue(draft: TaskEditorDraft, changes?: SchedulePatch): TaskManagerKey | 'incomplete' | undefined;
/** Only meaningful field changes enter the CAS edit; untouched timing is never rewritten. */
export declare function taskDraftPatch(draft: TaskEditorDraft, original: ScheduleCatalogEntry, intervalFromNow?: boolean): SchedulePatch;
/** Reapply changed field groups only after the user chooses to review a newer revision. */
export declare function reapplyTaskDraft(draft: TaskEditorDraft, latest: ScheduleCatalogEntry, changes: SchedulePatch): TaskEditorDraft;
/** Resolve the explicit destination, preserving an existing preset when the project is unchanged. */
export declare function draftTarget(draft: TaskEditorDraft, original?: ScheduleCatalogEntry): ScheduleTargetRequest;
/**
 * Render manual creation and inline edits with the existing task rule controls.
 * @param props.disabled - whether the fields reject input.
 * @param props.held - whether a disabled state keeps the enabled look because it lasts only one brief request.
 */
export declare function TaskEditorFields({ draft, onChange, projects, disabled, held, original, t }: {
    readonly draft: TaskEditorDraft;
    readonly onChange: (draft: TaskEditorDraft) => void;
    readonly projects: ProjectChoices;
    readonly disabled: boolean;
    readonly held: boolean;
    readonly original?: ScheduleCatalogEntry | undefined;
    readonly t: PropsLocale<'schedule.manager'>['t'];
}): ReactNode;
export {};
//# sourceMappingURL=TaskEditorFields.d.ts.map