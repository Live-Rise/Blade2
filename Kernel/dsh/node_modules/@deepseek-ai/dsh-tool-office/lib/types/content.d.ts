/**
 * Canonical Office tool results and their model-facing content.
 * @module @deepseek-ai/dsh-tool-office/content
 */
import type { ContentBlock } from '@deepseek-ai/dsh-llm';
import { type ImageAttachmentRef, type ImageMediaType } from '@deepseek-ai/dsh-attachment';
import type { OfficeConversionFormat, OfficeRecalculationFormat } from '@deepseek-ai/dsh-office-kit';
/**
 * One rendered image and the document location it shows. The attachment fields are those of the stored reference;
 * storage normalization keeps a rendered PNG or re-encodes it as JPEG or WebP, so `mediaType` names the stored encoding.
 */
export interface OfficeRenderedImage extends Pick<ImageAttachmentRef, 'bytes' | 'width' | 'height' | 'originalDimensions'> {
    mediaType: Exclude<ImageMediaType, 'image/gif'>;
    /** One-based page or slide; absent for worksheet areas. */
    page?: number;
    /** Worksheet of a worksheet area. */
    sheet?: string;
    /** A1 rectangle of a worksheet area. */
    range?: string;
    attachmentId: string;
}
/** Canonical result of `render`. */
export interface OfficeRenderValue {
    operation: 'render';
    /** Canonical absolute input path in the filesystem execution world. */
    path: string;
    /** Pages or slides of a document, or visible worksheets of a workbook. */
    pageCount: number;
    dpi: number;
    images: OfficeRenderedImage[];
    /** Requested font families that were substituted. */
    missingFonts: string[];
}
/** Output format of each file-producing operation. */
export interface OfficeFileFormats {
    convert: OfficeConversionFormat;
    recalculate: OfficeRecalculationFormat;
}
/** Canonical result of `convert` and `recalculate`. */
export interface OfficeFileValue<Operation extends 'convert' | 'recalculate' = 'convert' | 'recalculate'> {
    operation: Operation;
    /** Canonical absolute input path in the filesystem execution world. */
    path: string;
    /** Absolute path of the temporary output file in the filesystem execution world. */
    file: string;
    format: OfficeFileFormats[Operation];
    bytes: number;
    /** Page count the converter reports for the output. */
    pageCount?: number;
    /** Requested font families that were substituted. */
    missingFonts: string[];
}
/**
 * Project a render into a summary, then one label and image per rendered page or area.
 * @param value - canonical render result.
 * @returns text and image blocks in document order.
 */
export declare function renderImagesContent(value: OfficeRenderValue): ContentBlock[];
/**
 * Project a conversion or recalculation into its temporary file and the copy instruction.
 * @param value - canonical file result.
 * @returns one text block naming the temporary file.
 */
export declare function renderFileContent(value: OfficeFileValue): ContentBlock[];
//# sourceMappingURL=content.d.ts.map