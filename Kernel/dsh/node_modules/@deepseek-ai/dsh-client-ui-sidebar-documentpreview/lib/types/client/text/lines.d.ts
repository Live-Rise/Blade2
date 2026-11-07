import type { DocumentTextPage } from '../document/contract.ts';
import type { TextPage } from '../store.ts';
/**
 * Split a loaded page into its source lines.
 * @param page - source page.
 * @returns its lines, preserving one empty line but excluding a zero-line page.
 */
export declare function linesOf(page: TextPage): string[];
/** Loaded source page with its 1-based start position. */
export type LoadedPage = DocumentTextPage;
/**
 * Order loaded pages by source position.
 * @param pages - stored page table.
 * @returns pages in source order.
 */
export declare function loadedPages(pages: Readonly<Record<number, TextPage>>): LoadedPage[];
/**
 * Find the end of the loaded source prefix.
 * @param pages - ordered pages.
 * @returns the last loaded source line, or zero.
 */
export declare function lastLineLoaded(pages: readonly LoadedPage[]): number;
/**
 * Reveal a plain-text or highlighted source line.
 * @param body - scrolling document body or code-content viewport.
 * @param line - 1-based source line to reveal.
 * @returns The revealed line element, or undefined when the current renderer does not expose that line.
 */
export declare function scrollToLine(body: HTMLElement, line: number): HTMLElement | undefined;
/**
 * Mark a revealed line with a fading band. The band is a child of the scrollport,
 * not of the line, so a renderer that rebuilds its lines keeps the mark. It
 * replaces an earlier mark and removes itself when its animation ends or when
 * the scrollport is resized, because a resize can reflow the lines above it.
 * @param body - the scrollport that {@link scrollToLine} scrolled.
 * @param row - the line element it returned.
 * @param line - 1-based source line, recorded on the band.
 */
export declare function markLine(body: HTMLElement, row: HTMLElement, line: number): void;
/**
 * Remove the scrollport's line mark, for layout changes that move lines without resizing it.
 * @param body - the scrollport that may hold a mark.
 */
export declare function clearLineMark(body: HTMLElement): void;
//# sourceMappingURL=lines.d.ts.map