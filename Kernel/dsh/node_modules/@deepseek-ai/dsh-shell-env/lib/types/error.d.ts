/** Machine-routable shell environment failures. @module */
import { HarnessError } from '@deepseek-ai/dsh-error';
/**
 * Every {@link ShellEnvError} code:
 * - `SHELL_ENV_CONTRIBUTION_INVALID` — a registered contributor returned an undeclared key or a non-string value; the
 *   shell command does not run, and the contributing plugin must be fixed.
 */
export declare const SHELL_ENV_ERROR_CODES: readonly ["SHELL_ENV_CONTRIBUTION_INVALID"];
/** One code from {@link SHELL_ENV_ERROR_CODES}. */
export type ShellEnvErrorCode = typeof SHELL_ENV_ERROR_CODES[number];
/** Shell environment failure carrying a stable {@link ShellEnvErrorCode}. */
export declare class ShellEnvError extends HarnessError {
    readonly code: ShellEnvErrorCode;
    /**
     * @param message - names the contributor and the offending key.
     * @param code - the failure class consumers route on.
     */
    constructor(message: string, code: ShellEnvErrorCode);
}
//# sourceMappingURL=error.d.ts.map