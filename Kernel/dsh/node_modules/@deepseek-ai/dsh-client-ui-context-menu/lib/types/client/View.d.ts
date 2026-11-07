import type { MenuEntry } from '@deepseek-ai/dsh-client-ui-primitives';
import type { InjectFace, PropsStore, PropsRenderSlots } from '@deepseek-ai/dsh-client-ui-slots';
import type { Shortcuts, ShortcutCatalogEntry } from '@deepseek-ai/dsh-client-shortcuts/client';
import type { createMenuStore } from './store.ts';
import type { MenuActionView } from './types.ts';
/**
 * @param actions - plain presentation.
 * @param catalog - effective user shortcuts.
 * @param renderSlot - owner-authorized keyed icon renderer.
 * @returns Existing Menu entries.
 */
export declare function menuEntries(actions: readonly MenuActionView[], catalog: readonly ShortcutCatalogEntry[], renderSlot: PropsRenderSlots<'contextMenu.icon'>['renderSlot']): MenuEntry[];
/** Shared overlay callbacks. */
export interface MenuOverlayInjected {
    hooks: {
        catalog: Shortcuts['catalog'];
    };
    close: () => void;
    returnFocus?: () => HTMLElement | null;
    select: (id: string) => void;
}
type Props = PropsStore<ReturnType<typeof createMenuStore>> & PropsRenderSlots<'contextMenu.content' | 'contextMenu.icon'> & InjectFace<MenuOverlayInjected>;
/** @param props - registered state, target-specific legacy rows, and actions. @returns one menu and persistent feedback. */
export declare function MenuOverlay({ useStore, actions, renderSlot, close, select, returnFocus, useCatalog }: Props): import("react").JSX.Element;
export {};
//# sourceMappingURL=View.d.ts.map