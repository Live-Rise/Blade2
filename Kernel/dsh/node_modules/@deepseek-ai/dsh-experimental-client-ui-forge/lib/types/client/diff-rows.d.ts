/**
 * Drawn rows of one file diff: numbered patch lines, their side-by-side
 * pairing, and the unchanged lines between hunks, which stay collapsed until
 * the file's new side supplies them.
 */
import type { ForgeDiffFile, ForgeDiffHunk } from '../types.ts';
/** One drawn line with its number on each side that has it. */
export type DiffRow = {
    readonly kind: 'add';
    readonly old: null;
    readonly new: number;
    readonly text: string;
} | {
    readonly kind: 'del';
    readonly old: number;
    readonly new: null;
    readonly text: string;
} | {
    readonly kind: 'context';
    readonly old: number;
    readonly new: number;
    readonly text: string;
};
/** One side of a side-by-side row. */
export interface SplitCell {
    readonly kind: DiffRow['kind'];
    readonly no: number;
    readonly text: string;
}
/** One side-by-side row; a side without a line is null. */
export interface SplitRow {
    readonly left: SplitCell | null;
    readonly right: SplitCell | null;
}
/** A run of drawn rows, or unchanged lines drawn as one row that expands them. */
export type DiffSegment = {
    readonly kind: 'rows';
    readonly rows: readonly DiffRow[];
}
/**
 * `index` is the hunk the lines precede, or the hunk count for the lines after the last hunk;
 * `count` is null while the length of the file is unknown.
 */
 | {
    readonly kind: 'gap';
    readonly index: number;
    readonly count: number | null;
};
/**
 * Number the lines of a hunk. Context lines count on both sides, deletions on
 * the old side, and additions on the new side; a `\ No newline at end of file`
 * marker is not a line.
 * @param hunk - one hunk.
 * @returns the rows in patch order.
 */
export declare function hunkRows(hunk: ForgeDiffHunk): DiffRow[];
/**
 * Pair rows for the side-by-side view: each run of deletions sits beside the
 * run of additions that follows it, row by row, and context rows fill both sides.
 * @param rows - rows in patch order.
 * @returns the paired rows.
 */
export declare function splitRows(rows: readonly DiffRow[]): SplitRow[];
/**
 * Split file text into lines as a patch numbers them; a final line ending ends the last line.
 * @param text - the file text.
 * @returns the lines with any carriage returns kept.
 */
export declare function textLines(text: string): string[];
/**
 * Whether file text is the new side the hunks describe: every context and
 * added line sits at its number. A file changed after its diff was read fails.
 * @param lines - the file's lines.
 * @param hunks - hunks of the file's diff.
 * @returns whether the lines match.
 */
export declare function matchesHunks(lines: readonly string[], hunks: readonly ForgeDiffHunk[]): boolean;
/**
 * Lay out one file as runs of rows separated by collapsed unchanged lines.
 * Unchanged lines before a hunk, between hunks, and after the last hunk
 * collapse into one gap each; an open gap draws its lines from `lines` once
 * they are known. The lines after the last hunk are unknown until `lines`
 * arrives, unless the file is new, deleted, or its last hunk reaches its end.
 * @param file - one file of a diff.
 * @param lines - the file's new side, which {@link matchesHunks} accepted, or undefined while it has not been read.
 * @param open - indexes of the gaps the user expanded.
 * @returns the segments in file order.
 */
export declare function segmentsOf(file: ForgeDiffFile, lines: readonly string[] | undefined, open: ReadonlySet<number>): DiffSegment[];
/**
 * Count the rows a file's hunks draw.
 * @param file - one file of a diff.
 * @returns the number of context, added, and deleted lines in its hunks.
 */
export declare function drawnLines(file: ForgeDiffFile): number;
//# sourceMappingURL=diff-rows.d.ts.map