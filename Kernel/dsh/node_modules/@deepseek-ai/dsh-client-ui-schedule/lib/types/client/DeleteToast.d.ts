/**
 * Transient outcome notice for schedule deletions.
 *
 * Every deletion — the Tasks page detail, the session task tab, the
 * Session-header popover, and the Tasks page's removal of every ended task —
 * reports its settled outcome into one store, and one `shell.overlay` entry
 * renders it as the app-wide banner. The surfaces keep no in-place deletion
 * notice of their own, so an outcome outlives the panel or tab that asked for
 * it.
 */
import type { ReactNode } from 'react';
import type { HostObservable, InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { CatalogDeleteOutcome } from './catalog-source.ts';
import type { ClearEndedOutcome } from './clear-ended.ts';
/** One reported outcome; `seq` keys the banner so a re-show restarts it. */
export type DeleteToastState = {
    readonly kind: 'deleted' | 'deleteFailed';
    readonly seq: number;
} | {
    readonly kind: 'cleared';
    readonly count: number;
    readonly seq: number;
} | {
    readonly kind: 'clearFailed';
    readonly seq: number;
};
/** Store the deletion wiring reports into and the overlay entry reads. */
export interface DeleteToastSource {
    /** Current notice for the overlay entry, or null while none shows. */
    readonly hooks: {
        readonly toast: HostObservable<DeleteToastState | null>;
    };
    /**
     * Show the notice of one settled deletion; 'pending' raises nothing because
     * the deletion already in flight reports its own outcome.
     * @param outcome - resolution of `CatalogInjected.onDelete`.
     */
    readonly report: (outcome: CatalogDeleteOutcome) => void;
    /**
     * Show the notice of one settled bulk removal.
     * @param outcome - resolution of the management page's bulk removal callback.
     */
    readonly reportClear: (outcome: ClearEndedOutcome) => void;
    /** Clear the shown notice. */
    readonly dismiss: () => void;
}
/**
 * Create the one deletion-outcome store the overlay entry shows.
 * @returns the observable notice with its report and dismiss actions.
 */
export declare function createDeleteToastSource(): DeleteToastSource;
/** Props of the `shell.overlay` entry: the notice store and localized copy. */
export type ScheduleDeleteToastProps = PropsRuntime<'shell.overlay'> & PropsLocale<'schedule.manager'> & InjectFace<Omit<DeleteToastSource, 'report' | 'reportClear'>>;
/**
 * Render the current deletion notice: a success banner for a confirmed deletion
 * or settled bulk removal, a warning for one that could not be confirmed, or
 * nothing.
 * @param props - the notice hook, its dismissal, and the locale seat.
 * @returns the banner on display, or null.
 */
export declare function ScheduleDeleteToast({ useToast, dismiss, t }: ScheduleDeleteToastProps): ReactNode;
//# sourceMappingURL=DeleteToast.d.ts.map