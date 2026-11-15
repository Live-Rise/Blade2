/** Workspace-relative name and path discovery through packaged ripgrep. */
import { readRipgrepLines, spawnRipgrep } from '@deepseek-ai/dsh-util-ripgrep';
import { RemoteError } from '@deepseek-ai/dsh-typert-protocol';
/** VCS metadata never contributes files or inferred directories. */
const VCS_NAMES = ['.git', '.svn', '.hg', '.bzr', '.jj', '.sl'];
/** Quote names as literal rg glob components, not patterns. */
function exclusion(path) {
    const literal = path.replace(/[\\*?\[\]{}!]/g, '\\$&');
    return `--glob=!${path.includes('/') ? '/' : ''}${literal}`;
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
export async function searchWorkspace(subprocess, spec, signal) {
    const timeout = new AbortController();
    const timer = setTimeout(() => {
        timeout.abort(new RemoteError('workspace-file/search-failed', 'Workspace file search timed out; narrow the workspace or increase search.timeoutMs', {}));
    }, spec.timeoutMs);
    const combined = AbortSignal.any([signal, timeout.signal]);
    const needle = spec.query.toLowerCase();
    const byPath = spec.query.includes('/');
    const entries = new Map();
    const admit = (path, type) => {
        const name = path.slice(path.lastIndexOf('/') + 1);
        if ((byPath ? path : name).toLowerCase().includes(needle))
            entries.set(path, { path, type });
    };
    try {
        combined.throwIfAborted();
        let run;
        let stderr = '';
        try {
            const handle = await spawnRipgrep(subprocess, {
                argv: [
                    '--files', '--sort=path', '--null', '--path-separator=/',
                    ...spec.includeHidden ? ['--hidden'] : [],
                    ...spec.respectGitignore ? [] : ['--no-ignore'],
                    ...[...VCS_NAMES, ...spec.excludedPaths].map(exclusion),
                ],
                cwd: spec.cwd,
                stdio: { stdin: 'ignore', stdout: 'pipe', stderr: { maxBytes: spec.stderrMaxBytes } },
                graceMs: spec.graceMs,
                signal: combined,
            });
            let listed = 0;
            run = await readRipgrepLines(handle, '\0', (path) => {
                if (listed === spec.maxEntries)
                    return false;
                admit(path, 'file');
                let end = path.lastIndexOf('/');
                while (end !== -1) {
                    admit(path.slice(0, end), 'directory');
                    end = path.lastIndexOf('/', end - 1);
                }
                listed += 1;
                return true;
            });
            stderr = handle.collected.stderr?.readFrom(0).text.trim() ?? '';
        }
        catch (cause) {
            signal.throwIfAborted();
            timeout.signal.throwIfAborted();
            throw new RemoteError('workspace-file/search-failed', 'Could not launch or read workspace file search; the subprocess provider must be able to execute the packaged ripgrep binary', {}, { cause });
        }
        const { outcome, stopped } = run;
        combined.throwIfAborted();
        if (!stopped && (outcome.signal !== null || (outcome.exitCode !== 0 && outcome.exitCode !== 1 && outcome.exitCode !== 2))) {
            throw new RemoteError('workspace-file/search-failed', `Workspace file search exited with code ${outcome.exitCode} and signal ${outcome.signal}; check the subprocess provider${stderr.length > 0 ? `: ${stderr}` : ''}`, {});
        }
        const matches = [...entries.values()].sort((a, b) => {
            const depth = a.path.split('/').length - b.path.split('/').length;
            return depth || (a.path < b.path ? -1 : 1);
        });
        return {
            matches: matches.slice(0, spec.maxResults),
            truncated: matches.length > spec.maxResults,
            incomplete: stopped || outcome.exitCode === 2,
        };
    }
    finally {
        clearTimeout(timer);
    }
}
//# sourceMappingURL=search.js.map