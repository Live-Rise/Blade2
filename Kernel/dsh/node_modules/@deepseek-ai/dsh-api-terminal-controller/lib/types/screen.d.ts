/** Plain-text projection of a terminal emulator buffer for readers that cannot render control sequences. */
import type { IBuffer } from '@xterm/headless';
import type { TerminalScreenPage, TerminalScreenRequest } from './types.ts';
/**
 * Last buffer row that carries content: the cursor row or a later non-blank row.
 * @param buffer - active emulator buffer.
 * @returns absolute row index, or -1 for an empty buffer.
 */
export declare function lastContentRow(buffer: IBuffer): number;
/**
 * First row of the logical line that holds the cursor.
 * @param buffer - active emulator buffer.
 * @returns absolute index of the row where the cursor's wrapped line begins.
 */
export declare function cursorLineStart(buffer: IBuffer): number;
/**
 * Join wrapped rows into logical lines and trim each line's trailing cell padding.
 * @param buffer - active emulator buffer.
 * @param from - first absolute row, inclusive.
 * @param to - last absolute row, exclusive.
 * @returns logical lines in screen order; a range starting inside a wrapped line begins with that row.
 */
export declare function renderLines(buffer: IBuffer, from: number, to: number): string[];
/**
 * Select a newest-relative page and keep its newest bytes within the byte bound.
 * @param lines - logical lines in screen order.
 * @param request - newest-relative offset, line count and UTF-8 byte bound.
 * @returns the page; `truncated` reports dropped lines or a cut line within the selected range.
 */
export declare function pageLines(lines: readonly string[], request: TerminalScreenRequest): TerminalScreenPage;
//# sourceMappingURL=screen.d.ts.map