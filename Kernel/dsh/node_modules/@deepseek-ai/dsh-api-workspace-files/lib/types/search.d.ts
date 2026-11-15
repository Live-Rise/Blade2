import type { SubprocessRuntime } from '@deepseek-ai/dsh-subprocess';
import type { WorkspaceFileSearchResult } from './types.ts';
/** Deployment budgets on one search's results, listed files and process. */
export interface SearchLimits {
    /** Cap on returned matches; additional matches report truncation after the bounded scan. */
    readonly maxResults: number;
    /** Cap on files considered from rg output; an extra file terminates rg and reports the search incomplete. */
    readonly maxEntries: number;
    /** Search process termination grace in milliseconds. */
    readonly graceMs: number;
    /** Search deadline in milliseconds; expiry rejects rather than returning partial results. */
    readonly timeoutMs: number;
    /** Byte cap on retained search-process stderr diagnostics. */
    readonly stderrMaxBytes: number;
}
/** Resolved inputs of one search; the caller validates the root and trims the query. */
export interface SearchSpec extends SearchLimits {
    /** Canonical workspace root in the subprocess execution world. */
    readonly cwd: string;
    /** Non-empty query without surrounding whitespace. */
    readonly query: string;
    /** Literal names at any depth, or exact root-relative paths when containing `/`. */
    readonly excludedPaths: readonly string[];
    /** Whether hidden files and directories may participate. */
    readonly includeHidden: boolean;
    /** Whether ripgrep's native ignore rules apply. */
    readonly respectGitignore: boolean;
}
/**
 * Match names, or relative paths for queries containing `/`, without case.
 * Directories derive only from listed files; empty directories and links are
 * absent. Results sort by depth, then path, after scanning the bounded file set.
 * Native ignore handling and read failures belong to rg: exit 2 reports partial
 * results; launch and other execution failures reject with a RemoteError.
 * @param subprocess - the composed process spawner, co-located with the filesystem.
 * @param spec - validated root, query, filters and process budgets.
 * @param signal - cancellation; rejection preserves its exact reason.
 * @returns bounded shallow-first matches and independent truncation/incompleteness flags.
 */
export declare function searchWorkspace(subprocess: Pick<SubprocessRuntime, 'spawn'>, spec: SearchSpec, signal: AbortSignal): Promise<WorkspaceFileSearchResult>;
//# sourceMappingURL=search.d.ts.map