import type { ReactNode } from 'react';
import type { FactoryComponentPropsOf } from '@deepseek-ai/dsh-client-ui-slots';
import type { UseSidebarRightTabInfo } from '@deepseek-ai/dsh-client-ui-sidebar-right/client';
import type { FilesInjected } from './face.ts';
import type { createFilesStore } from './store.ts';
/** The tab whose pane's navigator this control toggles. */
export interface NavigatorToggleInput {
    readonly useTabInfo: UseSidebarRightTabInfo;
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface SlotFactoryMap {
        /** Show or hide the pane's workspace tree; absent without a working directory. */
        'sidebar.files.navigator.toggle': {
            scope: 'session';
            props: NavigatorToggleInput;
            store: ReturnType<typeof createFilesStore>;
            locale: 'sidebarFiles';
            inject: Pick<FilesInjected, 'toggleDrawn'>;
        };
    }
}
/**
 * Show or hide the pane's navigator.
 * @param props - the tab, the pane records, the face's registration, and copy.
 * @returns the toggle, or nothing without a workspace.
 */
export declare function NavigatorToggle({ useTabInfo, sessionId, useSessions, useStore, actions, toggleDrawn, t }: FactoryComponentPropsOf<'sidebar.files.navigator.toggle'>): ReactNode;
//# sourceMappingURL=NavigatorToggle.d.ts.map