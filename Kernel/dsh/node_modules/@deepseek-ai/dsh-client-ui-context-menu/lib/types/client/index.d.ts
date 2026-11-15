import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { ContextMenuButtonInjected } from './Button.tsx';
import type { ContextMenuButtonProps } from './types.ts';
import type { ContextMenus, ContextMenuTarget, ContextMenuIcon, UseContextMenu } from './types.ts';
export type { ContextMenus, ContextMenuTarget, ContextMenuTargetMap, MenuPoint, MenuAction, MenuActionView, MenuContribution, MenuResolution } from './types.ts';
export type { ContextMenuBindings, ContextMenuTriggerProps, UseContextMenu } from './types.ts';
export type { ContextMenuIcon, ContextMenuButtonProps } from './types.ts';
export { createMenuStore } from './store.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        contextMenus: ContextMenus;
    }
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface GlobalStandardProps {
        /** Menu bindings for explicit objects, available in every Slot scope. */
        useContextMenu: UseContextMenu;
    }
    interface SlotFactoryMap {
        /** Ellipsis button hidden only while its target kind has no registered contributions. */
        'contextMenu.button': {
            scope: 'root';
            props: ContextMenuButtonProps;
            inject: ContextMenuButtonInjected;
        };
    }
    interface SlotMap {
        /** Business-owned glyph or image selected by the action's icon key. */
        'contextMenu.icon': {
            kind: 'keyed';
            scope: 'root';
            inline: true;
            owner: {
                icon: ContextMenuIcon;
            };
        };
        /** Optional React rows for existing extension positions; data contributions need no renderer. */
        'contextMenu.content': {
            kind: 'keyed';
            scope: 'root';
            owner: {
                target: ContextMenuTarget;
                dismiss: () => void;
            };
        };
    }
    interface LocaleNamespaceMap {
        contextMenu: 'failed' | 'back';
    }
}
/** Maximum time to prepare each contribution before omitting its pending actions. */
export interface Config {
    resolutionTimeoutMs?: number;
}
/** Validated Client menu preparation budget. */
export declare const Config: z<Config>;
/** Required overlay and localization services. */
export declare const inject: string[];
/** @param ctx - plugin-owned Client context. @param config - validated preparation budget. */
export declare function apply(ctx: Context, config?: Config): void;
//# sourceMappingURL=index.d.ts.map