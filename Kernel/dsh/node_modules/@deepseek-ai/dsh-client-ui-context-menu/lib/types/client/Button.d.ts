import type { FactoryComponentPropsOf } from '@deepseek-ai/dsh-client-ui-slots';
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { ContextMenus, ContextMenuTarget, MenuPoint } from './types.ts';
/** Menu facts and the per-button toggle, bound by the factory registration. */
export interface ContextMenuButtonInjected {
    hooks: {
        /** Instance id of the shared button that opened the current popup; null for gestures and after dismissal. */
        opener: ObservableSnapshot<string | null>;
        registeredKinds: ContextMenus['registeredKinds'];
    };
    /**
     * Close the popup this button opened, or open a new one for its target.
     * @param button - instance id of the activated button.
     * @param target - captured object.
     * @param point - viewport anchor below the button.
     */
    toggle: (button: string, target: ContextMenuTarget, point: MenuPoint) => void;
}
/**
 * Render the menu trigger while its target kind has a registered contribution.
 * @param props - Localized owner data and framework-bound menu facts.
 * @returns An available target's menu trigger.
 */
export declare function ContextMenuButton({ target, label, className, size, useOpener, useRegisteredKinds, toggle }: FactoryComponentPropsOf<'contextMenu.button'>): import("react").JSX.Element | null;
//# sourceMappingURL=Button.d.ts.map