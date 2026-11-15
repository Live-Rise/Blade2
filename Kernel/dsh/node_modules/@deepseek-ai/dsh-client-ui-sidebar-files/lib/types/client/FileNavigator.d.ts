import type { ReactNode } from 'react';
import type { UseSidebarRightTabInfo } from '@deepseek-ai/dsh-client-ui-sidebar-right/client';
import type { FactoryComponentPropsOf } from '@deepseek-ai/dsh-client-ui-slots';
import type { FilesInjected } from './face.ts';
import { type createFilesStore } from './store.ts';
/** What a navigator occurrence is drawn for. */
export interface FileNavigatorInput {
    /** The tab the navigator is drawn beside, and its pane. */
    readonly useTabInfo: UseSidebarRightTabInfo;
    /** Drawn for the `files` page: always shown and filling the page. */
    readonly page: boolean;
    /** Ask the containing plugin to open a file. @param address - the file's resource address. */
    readonly onOpen: (address: string) => void;
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface SlotFactoryMap {
        /**
         * One pane's workspace tree with its width and show/hide state. The host renders it as a direct child of
         * the content row it shares with the body, and that row carries `data-files-navigator-row`, which the
         * filled layout needs to hide the covered body.
         */
        'sidebar.files.navigator': {
            scope: 'session';
            props: FileNavigatorInput;
            store: ReturnType<typeof createFilesStore>;
            locale: 'sidebarFiles';
            inject: FilesInjected;
        };
    }
}
/**
 * The navigator Factory's component.
 * @param props - the tab it is drawn for, the pane's store records, the tree's face, and copy.
 * @returns the navigator, or nothing while hidden or without a workspace.
 */
export declare function FileNavigator({ useTabInfo, page, onOpen, sessionId, useSessions, useStore, actions, useRowWidth, useCanOpen, useSearchPreferences, attach, placeBeside, hold, toggle, scrolled, bodyDrawn, currentDrawn, navigatorDrawn, setQuery, toggleSearch, retrySearch, setSearchPreferences, searchDrawn, t, renderFactorySlot, useContextMenu, }: FactoryComponentPropsOf<'sidebar.files.navigator'>): ReactNode;
//# sourceMappingURL=FileNavigator.d.ts.map