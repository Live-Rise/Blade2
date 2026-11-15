/** Pure Office row derivation from the tool name, frozen call slice, and settled result. @module */
import type { LocaleKeysOf } from '@deepseek-ai/dsh-client-ui-slots';
import type { ToolCallBlock } from './tool-call-model.ts';
type OfficeTitleKey = Extract<LocaleKeysOf<'conversation'>, 'tool.title.officeRender' | 'tool.title.officeConvert' | 'tool.title.officeRecalculate' | 'tool.title.generic'>;
/** The Office row's tool title, path summary, and openable input. */
export interface OfficeRowModel {
    titleKey: OfficeTitleKey;
    /** Input, plus the output format of a conversion or recalculation; empty until the input argument completes. */
    summary: string;
    /** Recorded absolute input path, or an absolute input argument while running. */
    filePath: string | undefined;
}
/**
 * Derive an Office row from wire values.
 * @param toolName - wire tool name used for the title and format summary.
 * @param block - preparing, running, or settled Office block.
 * @param cwd - Session workspace root used to shorten paths.
 * @param home - Host account home shown as `~`.
 * @returns the row title, summary, and openable path.
 */
export declare function officeRowModel(toolName: string, block: ToolCallBlock, cwd?: string, home?: string): OfficeRowModel;
export {};
//# sourceMappingURL=office-card-model.d.ts.map