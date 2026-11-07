/** Explicit blockquote recognition and decoding for quote presentation. */
/**
 * Remove one blockquote marker from each explicitly quoted line.
 * @param text - consecutive blockquote lines, with up to three leading spaces.
 * @returns excerpt text with original line endings normalized.
 */
export declare function quoteTextOf(text: string): string;
/** One blockquote span in original UTF-16 text, excluding its trailing line break. */
export interface MessageQuoteRange {
    readonly start: number;
    readonly end: number;
}
/**
 * Recognize consecutive explicitly marked Markdown blockquote lines.
 * @param text - complete original message text.
 * @returns ordered nonblank quote spans; inline greater-than signs remain ordinary text.
 */
export declare function messageQuoteRanges(text: string): readonly MessageQuoteRange[];
//# sourceMappingURL=quote-text.d.ts.map