/**
 * Office document families, accepted formats, and input-dependent argument rules.
 * @module @deepseek-ai/dsh-tool-office/formats
 */
import type { OfficeConversionFormat, OfficeRecalculationFormat } from '@deepseek-ai/dsh-office-kit';
/** Operation requested by one Office tool call. */
export type OfficeOperationName = 'render' | 'convert' | 'recalculate';
/** Document family selected by an input extension. */
export type OfficeFamily = 'text' | 'spreadsheet' | 'presentation' | 'pdf';
/** Conversion outputs per input family. */
export declare const CONVERSION_OUTPUTS: Readonly<Record<Exclude<OfficeFamily, 'pdf'>, readonly OfficeConversionFormat[]>>;
/** Every conversion or recalculation output format. */
export declare const OUTPUT_FORMATS: readonly ["pdf", "docx", "odt", "txt", "xlsx", "ods", "csv", "pptx", "odp"];
/** Every recalculation output format. */
export declare const RECALCULATION_FORMATS: readonly ["xlsx", "ods"];
/** Arguments for rendering document pages or worksheet areas. */
export interface OfficeRenderArgs {
    input: string;
    pages?: number[];
    sheet?: string;
    range?: string;
    dpi?: number;
}
/** Arguments for converting a document. */
export interface OfficeConvertArgs {
    input: string;
    format: OfficeConversionFormat;
    sheet?: string;
}
/** Arguments for recalculating a workbook. */
export interface OfficeRecalculateArgs {
    input: string;
    format: OfficeRecalculationFormat;
}
/** Model-facing Office tool name. */
export type OfficeToolName = 'office_render' | 'office_convert' | 'office_recalculate';
/**
 * Refuse argument keys the called tool does not declare, because the open parameter object would otherwise ignore
 * them; a key that another Office tool declares names that tool.
 * @param tool - called tool.
 * @param args - call arguments.
 * @param declared - parameter names of every Office tool.
 * @throws Error naming the undeclared key and the Office tools that accept it.
 */
export declare function checkParameterNames(tool: OfficeToolName, args: object, declared: Readonly<Record<OfficeToolName, readonly string[]>>): void;
/**
 * Lowercase file extension of a path without its dot.
 * @param path - resolved or model-supplied path.
 * @returns the extension, or an empty string when the name has none.
 */
export declare function extensionOf(path: string): string;
/**
 * Select the document family of an input for one operation.
 * @param operation - requested operation.
 * @param path - input path whose extension names the format.
 * @returns the family the operation accepts.
 * @throws Error naming the accepted formats when the operation does not accept this input.
 */
export declare function inputFamily(operation: OfficeOperationName, path: string): OfficeFamily;
/**
 * Check render selections that the tool schema subset cannot express.
 * @param args - schema-validated call arguments.
 * @param family - input family selected by {@link inputFamily}.
 * @throws Error naming the misplaced or malformed argument.
 */
export declare function checkRenderArguments(args: OfficeRenderArgs, family: OfficeFamily): void;
/**
 * Check conversion formats and worksheet selection against the input family.
 * @param args - schema-validated conversion arguments.
 * @param family - input family.
 * @throws Error naming the formats available for this input.
 */
export declare function checkConversionArguments(args: OfficeConvertArgs, family: Exclude<OfficeFamily, 'pdf'>): void;
//# sourceMappingURL=formats.d.ts.map