import type { ContextMenuTriggerProps } from '@deepseek-ai/dsh-client-ui-context-menu/client';
import type { ToolCallOwnerProps, ToolTreeProps } from '../../contract/slots.ts';
/** Card props: the owner payload plus the render site's locale seat (plain prop). */
export type GenericToolCardProps = ToolCallOwnerProps & {
    t: ToolTreeProps['t'];
    /** Bind the recorded file to the original Session. */
    getFileTriggerProps: (path: string) => ContextMenuTriggerProps;
};
/** @param props - current tool stage and locale. @returns its preparation or dispatched card. */
export declare function GenericToolCard({ toolName, block, cwd, home, openFile, getFileTriggerProps, inspect, useDisclosure, t, }: GenericToolCardProps): import("react").JSX.Element;
//# sourceMappingURL=GenericToolCard.d.ts.map