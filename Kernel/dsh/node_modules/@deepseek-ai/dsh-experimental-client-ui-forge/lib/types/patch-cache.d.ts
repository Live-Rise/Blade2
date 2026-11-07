/**
 * Change request patches kept in memory, so the changes view reads no diff
 * from the code host again while a change request keeps its head commit and base branch.
 */
import type { ChangeRequestKey } from '@deepseek-ai/dsh-experimental-forge';
/**
 * One patch per change request, bounded by the total UTF-8 size of the kept
 * patches; the least recently read patch leaves first.
 */
export declare class PatchCache {
    private readonly maxBytes;
    private readonly entries;
    private bytes;
    /** @param maxBytes - most UTF-8 bytes kept together; a larger patch is not kept. */
    constructor(maxBytes: number);
    /**
     * Read the patch kept for a change request at a version, marking it the most recently read.
     * @param key - the change request.
     * @param version - head commit and base branch the reader needs.
     * @returns the patch, or undefined when none is kept at that version.
     */
    get(key: ChangeRequestKey, version: string): string | undefined;
    /**
     * Keep a patch in place of the change request's earlier one, then drop the
     * least recently read patches until the rest fit within `maxBytes`.
     * @param key - the change request.
     * @param version - head commit and base branch the patch was read at.
     * @param patch - the patch.
     */
    set(key: ChangeRequestKey, version: string, patch: string): void;
    private remove;
}
//# sourceMappingURL=patch-cache.d.ts.map