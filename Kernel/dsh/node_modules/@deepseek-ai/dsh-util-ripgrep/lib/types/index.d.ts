import type { SubprocessHandle, SubprocessOutcome, SubprocessRuntime, SubprocessSpawnSpec } from '@deepseek-ai/dsh-subprocess';
/**
 * Resolve the packaged executable lazily, once per process. Single-file runtimes use an existing
 * `-rg` sidecar; Electron uses the unpacked ASAR path. Resolution failures reject
 * this call, not package loading. No system executable or remote fallback is used.
 * @returns the packaged executable's absolute path.
 */
export declare function resolveRgPath(): Promise<string>;
/**
 * Start packaged ripgrep with explicit subprocess settings. `spec.argv` contains
 * only rg arguments, without an executable. This function prepends the resolved
 * rg executable path and `--no-config`, so ambient configuration cannot enable a
 * preprocessor. The caller owns exit classification and process cleanup.
 * @param subprocess - the composed subprocess service.
 * @param spec - cwd, rg arguments (without the executable), stdio, signal and termination grace.
 * @returns the live subprocess handle; resolution and synchronous spawn failures reject.
 */
export declare function spawnRipgrep(subprocess: Pick<SubprocessRuntime, 'spawn'>, spec: SubprocessSpawnSpec): Promise<SubprocessHandle>;
/** Exit facts and whether the line consumer requested early termination. */
export interface RipgrepLinesResult {
    readonly outcome: SubprocessOutcome;
    readonly stopped: boolean;
}
/**
 * Read UTF-8 stdout records without retaining the complete output. A false
 * callback return terminates the process and suppresses remaining records.
 * Completion, callback failure and stream failure all await process quiescence;
 * provider failures reject. Exit codes and cancellation remain consumer-owned.
 * @param handle - an owned ripgrep process spawned with piped stdout.
 * Records are decoded as UTF-8, so bytes of a name that is not valid UTF-8
 * arrive replaced with U+FFFD.
 * @param separator - newline for text output, or NUL for file paths that may contain newlines.
 * @param onLine - synchronous record consumer; false requests early termination.
 * @returns exit facts after stdout and the managed process range settle.
 */
export declare function readRipgrepLines(handle: SubprocessHandle, separator: '\n' | '\0', onLine: (line: string) => boolean): Promise<RipgrepLinesResult>;
//# sourceMappingURL=index.d.ts.map