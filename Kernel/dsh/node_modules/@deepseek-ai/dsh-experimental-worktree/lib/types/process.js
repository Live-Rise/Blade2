/** Cancellable commands under the calling Session's existing file policy, whose stdout is collected within a bound or streamed. */
var __addDisposableResource = (this && this.__addDisposableResource) || function (env, value, async) {
    if (value !== null && value !== void 0) {
        if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
        var dispose, inner;
        if (async) {
            if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
            dispose = value[Symbol.asyncDispose];
        }
        if (dispose === void 0) {
            if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
            dispose = value[Symbol.dispose];
            if (async) inner = dispose;
        }
        if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
        if (inner) dispose = function() { try { inner.call(this); } catch (e) { return Promise.reject(e); } };
        env.stack.push({ value: value, dispose: dispose, async: async });
    }
    else if (async) {
        env.stack.push({ async: true });
    }
    return value;
};
var __disposeResources = (this && this.__disposeResources) || (function (SuppressedError) {
    return function (env) {
        function fail(e) {
            env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
            env.hasError = true;
        }
        var r, s = 0;
        function next() {
            while (r = env.stack.pop()) {
                try {
                    if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
                    if (r.dispose) {
                        var result = r.dispose.call(r.value);
                        if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) { fail(e); return next(); });
                    }
                    else s |= 1;
                }
                catch (e) {
                    fail(e);
                }
            }
            if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
            if (env.hasError) throw env.error;
        }
        return next();
    };
})(typeof SuppressedError === "function" ? SuppressedError : function (error, suppressed, message) {
    var e = new Error(message);
    return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
import { deadline } from '@deepseek-ai/dsh-timeout';
/** Clear inherited repository, attribute-tree, command-line configuration, and replacement-ref selectors. */
const GIT_ENV = Object.fromEntries([
    'GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY',
    'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_CONFIG', 'GIT_CONFIG_PARAMETERS', 'GIT_CONFIG_COUNT',
    'GIT_CEILING_DIRECTORIES', 'GIT_PREFIX', 'GIT_SHALLOW_FILE', 'GIT_GRAFT_FILE',
    'GIT_REPLACE_REF_BASE', 'GIT_ATTR_SOURCE',
].map(key => [key, undefined]));
/**
 * Run an argv without a shell and await termination of its managed process range.
 * @param ctx - subprocess and sandbox providers in the same execution world.
 * @param config - complete process resource limits.
 * @param policy - unchanged calling Session policy.
 * @param cwd - canonical process working directory.
 * @param argv - executable followed by literal arguments.
 * @param signal - operation cancellation.
 * @param allowedExitCodes - command-specific successful statuses.
 * @param env - command-local environment overrides, applied after Git selector removal.
 * @returns complete bounded stdout and the exit code.
 */
export async function runCommand(ctx, config, policy, cwd, argv, signal, allowedExitCodes = [0], env = {}) {
    const env_1 = { stack: [], error: void 0, hasError: false };
    try {
        const timeout = __addDisposableResource(env_1, deadline(signal, config.timeoutMs, 'WORKTREE_TIMEOUT'), false);
        const child = await spawnCommand(ctx, config, policy, cwd, argv, timeout.signal, env, { maxBytes: config.maxOutputBytes });
        try {
            const outcome = await child.done;
            timeout.signal.throwIfAborted();
            // The spawn spec above collects both streams.
            const captured = child.collected;
            const exitCode = allowedExitCode(outcome, captured.stderr, allowedExitCodes);
            const stdout = captured.stdout.readFrom(0);
            if (stdout.lossy)
                throw new Error('worktree command output exceeds maxOutputBytes');
            return { stdout: stdout.text, exitCode };
        }
        finally {
            child.terminate();
            await child.waitForExit();
        }
    }
    catch (e_1) {
        env_1.error = e_1;
        env_1.hasError = true;
    }
    finally {
        __disposeResources(env_1);
    }
}
/**
 * Run an argv as {@link runCommand} does, but pass each NUL-terminated stdout
 * field to `onField` as it arrives instead of retaining the output, so the size
 * of stdout is unbounded and `maxOutputBytes` limits only stderr. An
 * unterminated remainder is ignored. A false return from `onField` terminates
 * the command, whose outcome is then not checked, even when the provider
 * rejects it for a launch cancelled before acknowledgement; otherwise only
 * status 0 succeeds.
 * @param ctx - subprocess and sandbox providers in the same execution world.
 * @param config - complete process resource limits.
 * @param policy - unchanged calling Session policy.
 * @param cwd - canonical process working directory.
 * @param argv - executable followed by literal arguments.
 * @param signal - operation cancellation.
 * @param onField - synchronous consumer of each field in order; false ends the command.
 * @returns true when `onField` ended the command, false when it exited successfully first.
 */
export async function scanCommand(ctx, config, policy, cwd, argv, signal, onField) {
    const env_2 = { stack: [], error: void 0, hasError: false };
    try {
        const timeout = __addDisposableResource(env_2, deadline(signal, config.timeoutMs, 'WORKTREE_TIMEOUT'), false);
        const child = await spawnCommand(ctx, config, policy, cwd, argv, timeout.signal, {}, 'pipe');
        try {
            // The spawn spec above pipes stdout and collects stderr.
            const stdout = child.stdout;
            stdout.setEncoding('utf8');
            let pending = '';
            let stopped = false;
            for await (const chunk of stdout) {
                const fields = (pending + chunk).split('\0');
                pending = fields.splice(-1).join('');
                if (!fields.every(field => onField(field))) {
                    stopped = true;
                    break;
                }
            }
            if (stopped) {
                child.terminate();
                await Promise.allSettled([child.done]);
                timeout.signal.throwIfAborted();
                return true;
            }
            const outcome = await child.done;
            timeout.signal.throwIfAborted();
            allowedExitCode(outcome, child.collected.stderr, [0]);
            return false;
        }
        finally {
            child.terminate();
            await Promise.allSettled([child.done]);
            await child.waitForExit();
        }
    }
    catch (e_2) {
        env_2.error = e_2;
        env_2.hasError = true;
    }
    finally {
        __disposeResources(env_2);
    }
}
/** Resolve, confine, and start one command whose stderr is collected within `maxOutputBytes`. */
async function spawnCommand(ctx, config, policy, cwd, argv, signal, env, stdout) {
    signal.throwIfAborted();
    const program = await ctx.subprocess.resolveExecutable(argv[0], undefined, signal);
    const command = [program, ...argv.slice(1)];
    const wrapped = policy.mode === 'danger-full-access'
        ? command
        : (await ctx.sandbox.confine(command, { ...policy, mode: policy.mode }, signal)).argv;
    signal.throwIfAborted();
    return ctx.subprocess.spawn({
        argv: wrapped,
        cwd,
        env: { ...GIT_ENV, GIT_TERMINAL_PROMPT: '0', GIT_LFS_SKIP_SMUDGE: '1', GIT_NO_LAZY_FETCH: '1', ...env },
        signal,
        graceMs: config.graceMs,
        stdio: { stdin: 'ignore', stdout, stderr: { maxBytes: config.maxOutputBytes } },
    });
}
/**
 * The exit code of a command that exited with an allowed status.
 * @throws when the command was signalled or exited with another status, quoting its stderr.
 */
function allowedExitCode(outcome, stderr, allowedExitCodes) {
    if (outcome.exitCode === null || !allowedExitCodes.includes(outcome.exitCode)) {
        throw new Error(`worktree command failed (${outcome.signal ?? outcome.exitCode}): ${stderr.readFrom(0).text.trim()}`);
    }
    return outcome.exitCode;
}
//# sourceMappingURL=process.js.map