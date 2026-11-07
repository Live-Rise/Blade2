/** Single-popup viewing state and feedback retained outside trigger components. */
import { type EngineStoreHandle } from '@deepseek-ai/dsh-client-store';
import type { MenuActionView, MenuPoint, ContextMenuTarget } from './types.ts';
interface Notice {
    id: number;
    text: string;
    success: boolean;
    action?: {
        label: string;
        onClick: () => void;
    };
}
interface MenuState {
    occurrence: number;
    closing: boolean;
    replacing: boolean;
    target: ContextMenuTarget | null;
    point: MenuPoint;
    items: readonly MenuActionView[];
    notice: Notice | null;
}
type MenuActions = {
    open(draft: MenuState, target: ContextMenuTarget, point: MenuPoint): void;
    items(draft: MenuState, items: readonly MenuActionView[]): void;
    exit(draft: MenuState, replacing: boolean): void;
    close(draft: MenuState): void;
    notify(draft: MenuState, notice: Notice | null): void;
};
/**
 * Create the root popup store.
 * @returns the presentation and its mutation actions.
 */
export declare function createMenuStore(): EngineStoreHandle<MenuState, MenuActions>;
export {};
//# sourceMappingURL=store.d.ts.map