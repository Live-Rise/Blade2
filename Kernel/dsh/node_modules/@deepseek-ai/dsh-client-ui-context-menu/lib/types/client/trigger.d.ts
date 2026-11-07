import type { ContextMenuTriggerProps, MenuPoint, ContextMenuTarget } from './types.ts';
/**
 * Create stable bindings for the plugin's injected hook.
 * @param open - Open against the supplied captured target and viewport position.
 * @param active - Whether the plugin still owns its service.
 * @returns Ordinary trigger-prop factory; no React subscription or context is created.
 */
export declare function createContextMenuBindings<Target extends object = ContextMenuTarget>(open: (target: Target, point: MenuPoint) => void, active: () => boolean): {
    getTriggerProps: (target: Target | (() => Target) | undefined, handlers?: ContextMenuTriggerProps) => ContextMenuTriggerProps;
};
//# sourceMappingURL=trigger.d.ts.map