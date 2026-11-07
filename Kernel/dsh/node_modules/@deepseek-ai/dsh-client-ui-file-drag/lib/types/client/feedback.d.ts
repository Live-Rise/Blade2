import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { InjectFace } from '@deepseek-ai/dsh-client-ui-slots';
/** Localized transient failure, with identity for restarting its presentation lifetime. */
export interface FileDragFailure {
    readonly text: string;
    readonly sequence: number;
}
/** Registration-owned feedback source and dismissal callback. */
export interface FileDragFeedbackInjected {
    readonly hooks: {
        readonly failure: ObservableSnapshot<FileDragFailure | null>;
    };
    readonly dismiss: () => void;
}
/**
 * Keep transient failures visible after their originating source unmounts.
 * @param props - framework-bound failure source and dismissal callback.
 * @returns the current failure toast, or nothing.
 */
export declare function FileDragFeedback({ useFailure, dismiss }: InjectFace<FileDragFeedbackInjected>): import("react").JSX.Element | null;
//# sourceMappingURL=feedback.d.ts.map