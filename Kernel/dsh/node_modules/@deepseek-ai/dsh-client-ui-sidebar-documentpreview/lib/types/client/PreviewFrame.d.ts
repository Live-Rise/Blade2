import type { ReactNode } from 'react';
import type { PropsRenderFactories, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ContextMenuTriggerProps } from '@deepseek-ai/dsh-client-ui-context-menu/client';
/** The inputs both navigator Factory positions render from. */
type NavigatorSeatProps = Pick<PropsRuntime<'sidebar.right.pane.tab'>, 'useTabInfo'> & PropsRenderFactories;
/**
 * Props of {@link PreviewFrame}. `banner`, `tools`, `handoff`, and `children` are
 * ReactNode values passed between Components of this package; no slot owner
 * data or injected member carries them.
 */
export type PreviewFrameProps = NavigatorSeatProps & {
    /** The path the header shows. */
    readonly path: string;
    /** Context menu handlers bound to the header path. */
    readonly pathTrigger: ContextMenuTriggerProps;
    /** A notice above the header. */
    readonly banner?: ReactNode;
    /** Controls between the path and the file handoff. */
    readonly tools?: ReactNode;
    /** The contributions that hand the file to the Host, in the same position in every state, before the navigator toggle. */
    readonly handoff?: ReactNode;
    /** The content before the navigator. */
    readonly children: ReactNode;
    /** Data attributes forwarded to the root element. */
    readonly [attribute: `data-${string}`]: string | undefined;
};
/**
 * The text preview's layout for every preview state.
 * @param props - the header and content parts, the navigator inputs, and the root's data attributes.
 * @returns the preview root.
 */
export declare function PreviewFrame({ path, pathTrigger, banner, tools, handoff, children, useTabInfo, renderFactorySlot, ...data }: PreviewFrameProps): ReactNode;
export {};
//# sourceMappingURL=PreviewFrame.d.ts.map