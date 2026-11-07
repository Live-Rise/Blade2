/** Preserve existing tab-menu extensions under the clicked Session's retained scope. */
import type { PropsRuntime, PropsRenderSlots, InjectFace } from '@deepseek-ai/dsh-client-ui-slots';
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { SidebarSessionViewSnapshot } from '../session-views.ts';
type Props = PropsRuntime<'contextMenu.content'> & PropsRenderSlots<'sidebar.right.tab.menu.item'> & InjectFace<{
    hooks: {
        views: ObservableSnapshot<readonly SidebarSessionViewSnapshot[]>;
    };
}>;
/** @param props - clicked tab and already retained view. @returns existing plugin menu rows. */
export declare function TabMenuContent({ target, dismiss, useViews, SessionProvider, renderSlot }: Props): import("react").JSX.Element | null;
export {};
//# sourceMappingURL=TabMenuContent.d.ts.map