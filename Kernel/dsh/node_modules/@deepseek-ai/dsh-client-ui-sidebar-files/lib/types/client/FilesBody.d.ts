import type { ReactNode } from 'react';
import type { InjectFace, PropsLocale, PropsRenderFactories, PropsRenderSlots, PropsRuntime, PropsStore } from '@deepseek-ai/dsh-client-ui-slots';
import type { FilesInjected } from './face.ts';
import type { createFilesStore } from './store.ts';
/** The page's composed props: the tab it draws, its directory-actions seat, the pane records and the tree's face, and copy. */
export type FilesBodyProps = PropsRuntime<'sidebar.right.pane.tab'> & PropsRenderSlots<'sidebar.right.tab.files.actions'> & PropsRenderFactories & PropsStore<ReturnType<typeof createFilesStore>> & InjectFace<FilesInjected> & PropsLocale<'sidebarFiles'>;
/**
 * The `files` page body.
 * @param props - composed slot props.
 * @returns the header above the pane's navigator, or the workspace line.
 */
export declare function FilesBody(props: FilesBodyProps): ReactNode;
//# sourceMappingURL=FilesBody.d.ts.map