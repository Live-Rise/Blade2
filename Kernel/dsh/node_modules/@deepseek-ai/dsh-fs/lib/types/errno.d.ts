/**
 * Classification of host errno failures into the shared filesystem codes.
 * @module @deepseek-ai/dsh-fs/errno
 */
/**
 * Classify a host filesystem rejection by its errno `code`: ENOENT and ENOTDIR become `FS_NOT_FOUND`,
 * EACCES and EPERM `FS_PERMISSION_DENIED`, EISDIR (a directory where a file is required)
 * `FS_NOT_REGULAR_FILE`, and any other errno `FS_IO_ERROR`, each with the original error as `cause`.
 * Callers that give an errno a narrower meaning, such as absence during a probe, test for it first.
 * @param operation - verb shown in the diagnostic, such as `read`, `stat`, or `list`.
 * @param displayPath - caller-facing path shown in the diagnostic.
 * @param error - the caught value.
 * @returns the classified error; an `FsError` and a value without a string errno `code` (a defect) are returned unchanged.
 */
export declare function fsErrorFromErrno(operation: string, displayPath: string, error: unknown): unknown;
//# sourceMappingURL=errno.d.ts.map