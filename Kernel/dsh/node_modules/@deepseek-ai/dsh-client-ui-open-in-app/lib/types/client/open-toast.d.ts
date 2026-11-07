/** Native-open feedback that survives the initiating panel: mobile success and all failures. */
import type { ReactNode } from 'react';
import type { HostObservable, InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import { NS } from './locales.ts';
import type { OpenInAppOutcome } from './outcome.ts';
/** One announced gesture; `seq` keys the banner so a repeat restarts it. */
export type OpenInAppToastState = Exclude<OpenInAppOutcome, {
    kind: 'skipped';
}> & {
    readonly seq: number;
};
/** Store every contribution reports into and the overlay entry reads. */
export interface OpenInAppToastSource {
    /** Current notice for the overlay entry, or null while none shows. */
    readonly hooks: {
        readonly toast: HostObservable<OpenInAppToastState | null>;
    };
    /**
     * Announce failures on every device and acknowledged opens or reveals on mobile.
     * Desktop successes and skipped gestures leave the current notice unchanged.
     * @param outcome - settled native-open result.
     */
    readonly report: (outcome: OpenInAppOutcome) => void;
    /** Clear the shown notice. */
    readonly dismiss: () => void;
}
/**
 * Create the one outcome store the overlay entry shows.
 * @returns the observable notice with its report and dismiss actions.
 */
export declare function createOpenInAppToastSource(): OpenInAppToastSource;
/** Props of the `shell.overlay` entry: the notice store, its dismissal, and localized copy. */
export type OpenInAppToastProps = PropsRuntime<'shell.overlay'> & PropsLocale<typeof NS> & InjectFace<Omit<OpenInAppToastSource, 'report'>>;
/**
 * Render a mobile handoff confirmation or a native-open failure.
 * @param props - the notice hook, its dismissal, and the locale seat.
 * @returns the banner on display, or null.
 */
export declare function OpenInAppToast({ useToast, dismiss, t }: OpenInAppToastProps): ReactNode;
//# sourceMappingURL=open-toast.d.ts.map