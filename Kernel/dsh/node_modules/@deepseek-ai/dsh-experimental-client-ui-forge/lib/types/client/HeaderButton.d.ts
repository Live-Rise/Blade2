/** Session header entry that shows or hides the forge tab and marks change requests that need attention. */
import type { ReactNode } from 'react';
import type { HostObservable, InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import type { PanelSnapshot } from './source.ts';
/** Business face of the header entry. */
export interface HeaderButtonInjected {
    /** Panel snapshot and tab visibility of the Session; the renderer binds them as `usePanel` and `useVisible`. */
    readonly hooks: {
        readonly panel: HostObservable<PanelSnapshot>;
        readonly visible: HostObservable<boolean>;
    };
    /**
     * Hide the forge tab when it is visible, otherwise open or reveal it.
     * @param visible - whether the tab is visible now.
     */
    readonly toggle: (visible: boolean) => void;
}
/** Full props of the header entry. */
export type HeaderButtonProps = InjectFace<HeaderButtonInjected> & PropsLocale<typeof NS>;
/**
 * Render the header entry.
 * @param props - panel snapshot and visibility hooks, the toggle, and copy.
 * @returns the entry, or nothing outside a Git checkout without bound change requests.
 */
export declare function HeaderButton({ usePanel, useVisible, toggle, t }: HeaderButtonProps): ReactNode;
//# sourceMappingURL=HeaderButton.d.ts.map