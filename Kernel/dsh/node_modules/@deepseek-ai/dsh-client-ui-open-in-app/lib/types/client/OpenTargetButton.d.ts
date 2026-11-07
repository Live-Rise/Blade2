import type { ReactNode } from 'react';
import type { ShortcutCatalogEntry } from '@deepseek-ai/dsh-client-shortcuts/client';
import type { TranslateNS } from '@deepseek-ai/dsh-client-ui-slots';
import type { OpenInAppOutcome } from './outcome.ts';
import type { NS } from './locales.ts';
/** Application metadata supplied by either the directory catalog or a file association query. */
export interface OpenTargetApplication {
    readonly id: string;
    readonly name: string;
    readonly icon: string | null;
}
/** Opening intent; default selection remains owned by the target's adapter. */
export type OpenTargetOperation = {
    readonly kind: 'default';
} | {
    readonly kind: 'application';
    readonly id: string;
} | {
    readonly kind: 'reveal';
};
/** Seat a contribution sends one settled gesture to for app-wide announcement. */
export interface OpenTargetReporter {
    /** Report the settled outcome; `{ kind: 'skipped' }` announces nothing. */
    readonly report: (outcome: OpenInAppOutcome) => void;
}
/** Inputs shared by both target adapters and the file empty-state action. */
export interface OpenTargetButtonProps extends OpenTargetReporter {
    readonly shortcut?: Pick<ShortcutCatalogEntry, 'keys' | 'aria'> | undefined;
    readonly kind: 'file' | 'directory';
    readonly applications: readonly OpenTargetApplication[];
    readonly defaultId: string | undefined;
    readonly failed: boolean;
    readonly busy?: boolean;
    readonly loading?: boolean;
    readonly prominent?: boolean;
    readonly t: TranslateNS<typeof NS>;
    /** Resolve every gesture to an outcome, including Host and transport failures; never reject. */
    readonly execute: (operation: OpenTargetOperation) => Promise<OpenInAppOutcome>;
    readonly refresh?: () => void;
}
/**
 * Serialize gestures and report the outcome each one settles on to the app-wide
 * announcer, which outlives this control and the panel holding it.
 * @param execute - target adapter that settles on the gesture's outcome.
 * @param report - announcement seat the settled outcome is sent to.
 * @returns the pending state and the guarded action callback.
 */
export declare function useOpenTargetGesture(execute: OpenTargetButtonProps['execute'], report: OpenTargetButtonProps['report']): {
    pending: boolean;
    act: (operation: OpenTargetOperation) => void;
};
/**
 * Render identical split buttons for files and directories. File reveal always
 * stays last; it is the default only when no application is registered.
 * @param props - target applications, default selection, and operations.
 * @returns the control alone: its settled outcomes are announced by the seat it reports to.
 */
export declare function OpenTargetButton(props: OpenTargetButtonProps): ReactNode;
//# sourceMappingURL=OpenTargetButton.d.ts.map