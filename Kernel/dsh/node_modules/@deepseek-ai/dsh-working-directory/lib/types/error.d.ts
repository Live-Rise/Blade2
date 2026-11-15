/** Machine-routable Session working-directory failures. @module */
import { HarnessError } from '@deepseek-ai/dsh-error';
/**
 * Every {@link WorkingDirectoryError} code:
 * - `WORKING_DIRECTORY_INVALID` — a requested directory is empty or otherwise malformed.
 * - `WORKING_DIRECTORY_NOT_FOUND` — the requested directory does not exist or is not a directory; the caller picks another path.
 * - `WORKING_DIRECTORY_UNAVAILABLE` — neither the current directory nor the original project exists, or the owning service was disposed.
 */
export declare const WORKING_DIRECTORY_ERROR_CODES: readonly ["WORKING_DIRECTORY_INVALID", "WORKING_DIRECTORY_NOT_FOUND", "WORKING_DIRECTORY_UNAVAILABLE"];
/** One code from {@link WORKING_DIRECTORY_ERROR_CODES}. */
export type WorkingDirectoryErrorCode = typeof WORKING_DIRECTORY_ERROR_CODES[number];
/** Working-directory failure carrying a stable {@link WorkingDirectoryErrorCode}. */
export declare class WorkingDirectoryError extends HarnessError {
    readonly code: WorkingDirectoryErrorCode;
    /**
     * @param message - names the directory and the safe next step.
     * @param code - the failure class consumers route on.
     * @param options - standard `cause` chaining.
     */
    constructor(message: string, code: WorkingDirectoryErrorCode, options?: ErrorOptions);
}
//# sourceMappingURL=error.d.ts.map