/** Unsaved task forms survive navigation without persisting business records. */
import { type EngineStoreHandle } from '@deepseek-ai/dsh-client-store';
import type { PropsStore } from '@deepseek-ai/dsh-client-ui-slots';
import type { ScheduleCatalogEntry } from '@deepseek-ai/dsh-schedule/client';
import type { TaskEditorDraft } from './TaskEditorFields.tsx';
import type { HostEditFailure, TaskCreateOutcome } from './TaskDetail.tsx';
type EditorFailure = HostEditFailure | Extract<TaskCreateOutcome, {
    failed: string;
}>['failed'];
interface EditorDraft {
    readonly draft: TaskEditorDraft;
    readonly base: ScheduleCatalogEntry | undefined;
    readonly initial?: TaskEditorDraft;
    readonly pending?: boolean;
    readonly failure?: EditorFailure;
}
interface EditorState {
    drafts: Record<string, EditorDraft | undefined>;
}
declare const editorActions: {
    setDraft: (state: EditorState, key: string, draft: EditorDraft) => void;
    clearDraft: (state: EditorState, key: string) => void;
};
/**
 * Declare drafts retained by the framework for one entry's scope.
 * @returns the entry's store handle.
 */
export declare function createEditorStore(): EngineStoreHandle<EditorState, typeof editorActions>;
/** Framework-bound selectors and actions; components never receive the store itself. */
export type TaskEditorStoreProps = PropsStore<ReturnType<typeof createEditorStore>>;
export {};
//# sourceMappingURL=editor-store.d.ts.map