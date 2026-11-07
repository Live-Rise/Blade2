import type { ToolCallViewProps } from '@deepseek-ai/dsh-client-ui-tool/client';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
type ProposeRowProps = ToolCallViewProps & PropsLocale<'actionCards'>;
/**
 * Render a `propose_action` call from its recorded arguments and result. Only a refused call expands, to show
 * why no card followed the answer.
 * @param props - tool call and localized status copy.
 * @returns the status row.
 */
export declare function ProposeRow(props: ProposeRowProps): import("react").JSX.Element;
export {};
//# sourceMappingURL=ProposeRow.d.ts.map