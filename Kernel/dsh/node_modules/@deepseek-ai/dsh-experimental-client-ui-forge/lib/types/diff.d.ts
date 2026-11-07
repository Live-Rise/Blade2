/** Unified patch parsing into the panel's browser-safe diff model, and the diff and size of an untracked file. */
import type { ForgeChangedFile, ForgeDiffFile } from './types.ts';
/**
 * Parse a Git-style unified patch, as `git diff` or a code host prints it,
 * with `a/` and `b/` path prefixes. Hunk ranges follow jsdiff: a range that
 * covers no line starts at the line it comes before.
 * @param patch - the patch text.
 * @returns one file per patch section in patch order.
 */
export declare function parseDiff(patch: string): ForgeDiffFile[];
/**
 * Describe an untracked file as added in full.
 * @param path - path relative to the repository root.
 * @param content - the file's bytes.
 * @returns the file with one hunk of added lines, none when it is empty, or none and `binary` when its content is not text.
 */
export declare function untrackedFile(path: string, content: Uint8Array): ForgeDiffFile;
/**
 * Decode file content as text the way Git decides it is not binary: no NUL
 * byte among the first 8000 bytes, and, here, valid UTF-8 throughout.
 * @param content - the bytes.
 * @returns the text, or undefined for binary content.
 */
export declare function decodeText(content: Uint8Array): string | undefined;
/**
 * Parse `git diff --numstat -z` output, in which a rename prints its counts,
 * an empty path, and then the old and new paths as separate fields.
 * @param output - the command's standard output.
 * @returns one file per record, named by its new path; a binary file counts no lines.
 */
export declare function parseNumstat(output: string): ForgeChangedFile[];
//# sourceMappingURL=diff.d.ts.map