import type { PropsRenderSlots, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
type Props = Pick<PropsRuntime<'contextMenu.content'>, 'target' | 'dismiss'> & PropsRenderSlots<'sidebar.workspaces.session.menu.item'>;
/** @param props - captured Session and the existing extension position. @returns legacy component contributions. */
export declare function SessionMenu({ target, dismiss, renderSlot }: Props): import("react").ReactNode;
export {};
//# sourceMappingURL=SessionMenu.d.ts.map