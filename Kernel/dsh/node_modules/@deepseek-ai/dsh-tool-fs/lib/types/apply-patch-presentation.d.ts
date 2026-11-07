import type { DiffCallView, DiffResultView, ToolResult } from '@deepseek-ai/dsh-tools';
/**
 * Preview the requested operations without resolving paths or reading the filesystem.
 * A deletion has no diff text to preview and appears only among the locations.
 * @param patch - raw patch argument.
 * @returns a diff call view, or undefined when syntax is invalid.
 */
export declare function presentPatchCall(patch: string): DiffCallView | undefined;
/**
 * Present applied metadata; failed or missing metadata uses generic result rendering.
 * @param result - live or replayed tool result.
 * @returns known applied hunks with a stopped title where appropriate.
 */
export declare function presentPatchResult(result: ToolResult): DiffResultView | undefined;
//# sourceMappingURL=apply-patch-presentation.d.ts.map