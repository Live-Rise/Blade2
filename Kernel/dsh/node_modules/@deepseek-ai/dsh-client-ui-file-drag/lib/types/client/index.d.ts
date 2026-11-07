/** Cordis-owned file transfers and localized feedback shared across independent entries. */
import type { Context } from '@deepseek-ai/cordis';
import type { FileDragActions } from '@deepseek-ai/dsh-client-ui-primitives';
/** Application-local file preparation and drop resolution. */
export interface FileDragService {
    readonly actions: FileDragActions;
    /** @param transfer - browser drop payload. @returns original file for a matching application-local token. */
    readonly read: (transfer: DataTransfer) => File | undefined;
}
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** File transfer ownership scoped to this plugin's lifetime. */
        fileDrag: FileDragService;
    }
}
/** Registry and locale used by the independent feedback overlay. */
export declare const inject: string[];
/** @param ctx - client plugin context carrying slots and locale. */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map