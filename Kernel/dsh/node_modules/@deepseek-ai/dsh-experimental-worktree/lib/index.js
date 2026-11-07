import { Service } from "@deepseek-ai/cordis";
import z from "@deepseek-ai/schemastery";
import { MAX_TIMER_DELAY_MS, deadline } from "@deepseek-ai/dsh-timeout";
import { randomBytes } from "node:crypto";
//#region lib/types/directory.js
/** Static child program for exclusive checkout-directory allocation inside the process sandbox. */
/**
* The child receives the configured root and destination as literal argv. Existing roots retain
* their files; only the process that creates a root writes its self-ignoring .gitignore.
*/
const PREPARE_DIRECTORY = `
import fs from 'node:fs/promises';
import path from 'node:path';
async function main() {
  const [root, destination] = process.argv.slice(1);
  await fs.mkdir(path.dirname(root), { recursive: true });
  let created = false;
  try {
    await fs.mkdir(root);
    created = true;
  } catch (error) {
    if (error.code !== 'EEXIST' || !(await fs.stat(root)).isDirectory()) throw error;
  }
  if (created) await fs.writeFile(path.join(root, '.gitignore'), '*\\n', { flag: 'wx' });
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.mkdir(destination);
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
`;
//#endregion
//#region lib/types/name.js
/** Generated worktree names that lead back to the Session that created them. */
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
/**
* Generate a branch and checkout name: the prefix, the first eight hexadecimal digits of the
* first UUID in the Session id, a hyphen, and four random hexadecimal digits. A Session id
* without a UUID gets eight random hexadecimal digits after the prefix instead.
* @param prefix - validated name prefix.
* @param sessionId - id of the Session the worktree is created for.
* @returns the generated name; it is unique only with high probability, so creation still rejects an existing branch.
*/
function generatedWorktreeName(prefix, sessionId) {
	const uuid = UUID.exec(sessionId)?.[0];
	return uuid === void 0 ? `${prefix}${randomBytes(4).toString("hex")}` : `${prefix}${uuid.slice(0, 8).toLowerCase()}-${randomBytes(2).toString("hex")}`;
}
//#endregion
//#region lib/types/process.js
/** Bounded, cancellable commands under the calling Session's existing file policy. */
var __addDisposableResource = function(env, value, async) {
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
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Clear inherited repository, attribute-tree, command-line configuration, and replacement-ref selectors. */
const GIT_ENV = Object.fromEntries([
	"GIT_DIR",
	"GIT_WORK_TREE",
	"GIT_COMMON_DIR",
	"GIT_INDEX_FILE",
	"GIT_OBJECT_DIRECTORY",
	"GIT_ALTERNATE_OBJECT_DIRECTORIES",
	"GIT_CONFIG",
	"GIT_CONFIG_PARAMETERS",
	"GIT_CONFIG_COUNT",
	"GIT_CEILING_DIRECTORIES",
	"GIT_PREFIX",
	"GIT_SHALLOW_FILE",
	"GIT_GRAFT_FILE",
	"GIT_REPLACE_REF_BASE",
	"GIT_ATTR_SOURCE"
].map((key) => [key, void 0]));
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
async function runCommand(ctx, config, policy, cwd, argv, signal, allowedExitCodes = [0], env = {}) {
	const env_1 = {
		stack: [],
		error: void 0,
		hasError: false
	};
	try {
		const timeout = __addDisposableResource(env_1, deadline(signal, config.timeoutMs, "WORKTREE_TIMEOUT"), false);
		timeout.signal.throwIfAborted();
		const command = [await ctx.subprocess.resolveExecutable(argv[0], void 0, timeout.signal), ...argv.slice(1)];
		const wrapped = policy.mode === "danger-full-access" ? command : (await ctx.sandbox.confine(command, {
			...policy,
			mode: policy.mode
		}, timeout.signal)).argv;
		timeout.signal.throwIfAborted();
		const child = ctx.subprocess.spawn({
			argv: wrapped,
			cwd,
			env: {
				...GIT_ENV,
				GIT_TERMINAL_PROMPT: "0",
				GIT_LFS_SKIP_SMUDGE: "1",
				GIT_NO_LAZY_FETCH: "1",
				...env
			},
			signal: timeout.signal,
			graceMs: config.graceMs,
			stdio: {
				stdin: "ignore",
				stdout: { maxBytes: config.maxOutputBytes },
				stderr: { maxBytes: config.maxOutputBytes }
			}
		});
		try {
			const outcome = await child.done;
			timeout.signal.throwIfAborted();
			const captured = child.collected;
			const stdout = captured.stdout.readFrom(0);
			const stderr = captured.stderr.readFrom(0);
			if (outcome.exitCode === null || !allowedExitCodes.includes(outcome.exitCode)) throw new Error(`worktree command failed (${outcome.signal ?? outcome.exitCode}): ${stderr.text.trim()}`);
			if (stdout.lossy) throw new Error("worktree command output exceeds maxOutputBytes");
			return {
				stdout: stdout.text,
				exitCode: outcome.exitCode
			};
		} finally {
			child.terminate();
			await child.waitForExit();
		}
	} catch (e_1) {
		env_1.error = e_1;
		env_1.hasError = true;
	} finally {
		__disposeResources(env_1);
	}
}
//#endregion
//#region lib/types/index.js
/** Creates a new Git branch and checkout, then changes the calling Session's working directory; restores a removed checkout. */
/** Reject path traversal independently of Git's branch-name grammar. */
function relativePath(value) {
	return value.length > 0 && !value.includes("\\") && !value.includes(":") && value.split("/").every((part) => part.length > 0 && part !== "." && part !== "..");
}
/** Creates retained Git worktrees under the mounted filesystem and sandbox providers. */
var WorktreeService = class extends Service {
	static inject = [
		"workingDirectory",
		"fs",
		"subprocess",
		"sandbox",
		"sandboxPolicy"
	];
	static Config = z.object({
		directory: z.string().default(".agents/worktrees"),
		namePrefix: z.string().default("dsh-"),
		gitCommand: z.string().default("git"),
		nodeCommand: z.string().default("node"),
		timeoutMs: z.number().default(6e4),
		graceMs: z.number().default(3e3),
		maxOutputBytes: z.number().default(65536)
	});
	config;
	lifetime = new AbortController();
	pending = /* @__PURE__ */ new Set();
	constructor(ctx, config) {
		super(ctx, "worktrees");
		this.config = config;
		if (!relativePath(this.config.directory)) throw new Error("worktree: directory must be a relative directory without dot segments");
		if (!relativePath(`${this.config.namePrefix}name`) || this.config.namePrefix.startsWith("-")) throw new Error("worktree: namePrefix must produce a relative branch name");
		for (const key of ["gitCommand", "nodeCommand"]) if (this.config[key].trim().length === 0) throw new Error(`worktree: ${key} must not be empty`);
		for (const key of [
			"timeoutMs",
			"graceMs",
			"maxOutputBytes"
		]) {
			const value = this.config[key];
			if (!Number.isSafeInteger(value) || value <= 0 || value > MAX_TIMER_DELAY_MS) throw new Error(`worktree: ${key} must be a positive integer no greater than ${MAX_TIMER_DELAY_MS}`);
		}
		ctx.effect(() => async () => {
			this.lifetime.abort(/* @__PURE__ */ new Error("worktree service disposed"));
			await Promise.allSettled(this.pending);
		});
	}
	/**
	* Create a fresh branch and checkout at a pinned local revision, then enter it.
	* Existing branches or checkout paths fail. Uncommitted files stay in the source checkout.
	* Checkout disables configured clean, smudge, and process filters without changing Git config.
	* Repository-local replacement refs still apply; baseCommit reports the resolved object name.
	* The new checkout becomes current only after Git setup succeeds. Failures may retain newly
	* allocated Git/filesystem artifacts; no branch or checkout is removed automatically.
	* @param agent - caller whose current directory selects the source repository and file policy.
	* @param request - optional new name and local revision; defaults are generated name and HEAD.
	* @param signal - cancellation of lookup, creation, and working-directory publication.
	* @param onPhase - receives each stage before it begins; it must not throw.
	* @returns canonical checkout path, branch name, pinned commit, and source repository root.
	*/
	async create(agent, request = {}, signal, onPhase) {
		const operationSignal = signal === void 0 ? this.lifetime.signal : AbortSignal.any([this.lifetime.signal, signal]);
		operationSignal.throwIfAborted();
		const operation = this.createCheckout(agent, this.resolve(agent, request), operationSignal, onPhase);
		this.pending.add(operation);
		try {
			return await operation;
		} finally {
			this.pending.delete(operation);
		}
	}
	/**
	* Check out a removed worktree again at its recorded directory: its branch
	* when the branch still exists, otherwise a new branch of the same name at
	* `baseCommit`. Only the missing target's unlocked registration is removed, and an
	* existing directory fails. Changes that were never committed are not
	* recovered. Checkout disables configured clean, smudge, and process filters,
	* as creation does. The calling Session's working directory does not change.
	* Failures may retain newly allocated Git/filesystem artifacts.
	* @param agent - caller whose sandbox policy confines the commands.
	* @param request - recorded directory, branch, base commit, and source repository root.
	* @param signal - cancellation of the restoration.
	* @param onPhase - receives each stage before it begins; it must not throw.
	* @returns the directory and whether the branch was created again.
	*/
	async restore(agent, request, signal, onPhase) {
		const operationSignal = signal === void 0 ? this.lifetime.signal : AbortSignal.any([this.lifetime.signal, signal]);
		operationSignal.throwIfAborted();
		const operation = this.restoreCheckout(agent, request, operationSignal, onPhase);
		this.pending.add(operation);
		try {
			return await operation;
		} finally {
			this.pending.delete(operation);
		}
	}
	resolve(agent, request) {
		const branch = request.name ?? generatedWorktreeName(this.config.namePrefix, agent.session.id);
		if (!relativePath(branch) || branch.startsWith("-")) throw new Error("worktree name must be a relative Git branch name");
		const revision = request.from ?? "HEAD";
		if (revision.length === 0) throw new Error("worktree from must name a local commit, branch, or tag");
		return {
			branch,
			revision
		};
	}
	async createCheckout(agent, { branch, revision }, signal, onPhase) {
		onPhase?.("prepare");
		const cwd = await this.ctx.workingDirectory.ensure(agent, signal);
		const policy = this.ctx.sandboxPolicy.resolve({ session: agent.session });
		const run = this.gitRunner(policy, signal);
		await run(cwd, ["check-ref-format", `refs/heads/${branch}`]);
		const rootOutput = await run(cwd, ["rev-parse", "--show-toplevel"]);
		const rootTarget = await this.ctx.fs.resolve(rootOutput.stdout.replace(/\r?\n$/, ""), {
			cwd,
			signal
		});
		const repositoryRoot = this.ctx.fs.processPath(rootTarget);
		const baseCommit = (await run(repositoryRoot, [
			"rev-parse",
			"--verify",
			"--end-of-options",
			`${revision}^{commit}`
		])).stdout.trim();
		if ((await run(repositoryRoot, [
			"show-ref",
			"--verify",
			"--quiet",
			`refs/heads/${branch}`
		], [0, 1])).exitCode === 0) throw new Error(`worktree branch already exists: ${branch}`);
		const pool = await this.ctx.fs.resolve(this.config.directory, {
			cwd: repositoryRoot,
			signal
		});
		const poolPath = this.ctx.fs.processPath(pool);
		const destination = await this.ctx.fs.resolve(branch, {
			cwd: poolPath,
			signal
		});
		const path = this.ctx.fs.processPath(destination);
		if (await this.ctx.fs.lstat(path, void 0, signal) !== void 0) throw new Error(`worktree path already exists: ${path}`);
		try {
			await this.prepareDirectory(policy, repositoryRoot, poolPath, path, signal);
			await run(repositoryRoot, [
				"--work-tree",
				repositoryRoot,
				"worktree",
				"add",
				"--no-checkout",
				"-b",
				branch,
				"--",
				path,
				baseCommit
			]);
			const overrides = await filterOverrides(run, path);
			onPhase?.("checkout");
			await run(path, [
				"--work-tree",
				path,
				"reset",
				"--hard",
				"--no-recurse-submodules",
				baseCommit
			], void 0, overrides);
			return {
				path: await this.ctx.workingDirectory.set(agent, path, signal),
				branch,
				baseCommit,
				repositoryRoot
			};
		} catch (error) {
			throw new Error(`Could not finish worktree ${branch} at ${path}. Any created checkout and branch are retained. ${error instanceof Error ? error.message : String(error)}`, { cause: error });
		}
	}
	async restoreCheckout(agent, { path, branch, baseCommit, repositoryRoot }, signal, onPhase) {
		onPhase?.("prepare");
		const policy = this.ctx.sandboxPolicy.resolve({ session: agent.session });
		const run = this.gitRunner(policy, signal);
		if (await this.ctx.fs.lstat(path, void 0, signal) !== void 0) throw new Error(`worktree path already exists: ${path}`);
		const root = await this.ctx.fs.resolve(repositoryRoot, { signal });
		const pool = await this.ctx.fs.resolve(this.config.directory, {
			cwd: repositoryRoot,
			signal
		});
		const target = await this.ctx.fs.resolve(path, {
			cwd: repositoryRoot,
			signal
		});
		if (this.ctx.fs.processPath(root) !== repositoryRoot || this.ctx.fs.processPath(target) !== path || !this.ctx.fs.contains(root, pool) || !this.ctx.fs.contains(pool, target) || pool.targetKey === target.targetKey) throw new Error("worktree restoration must use a canonical directory inside the configured pool");
		if (!relativePath(branch) || branch.startsWith("-")) throw new Error("worktree branch must be a relative Git branch name");
		if (!/^(?:[a-f\d]{40}|[a-f\d]{64})$/i.test(baseCommit)) throw new Error("worktree baseCommit must be a full commit object name");
		await run(repositoryRoot, ["check-ref-format", `refs/heads/${branch}`]);
		const top = await run(repositoryRoot, ["rev-parse", "--show-toplevel"]);
		if ((await this.ctx.fs.resolve(top.stdout.replace(/\r?\n$/, ""), {
			cwd: repositoryRoot,
			signal
		})).targetKey !== root.targetKey) throw new Error("worktree repositoryRoot must be the source checkout root");
		await run(repositoryRoot, [
			"rev-parse",
			"--verify",
			"--end-of-options",
			`${baseCommit}^{commit}`
		]);
		const registered = await run(repositoryRoot, [
			"worktree",
			"list",
			"--porcelain",
			"-z"
		]);
		for (const entry of registered.stdout.split("\0\0")) {
			const fields = entry.split("\0");
			const recordedPath = fields.find((field) => field.startsWith("worktree "))?.slice(9);
			if (recordedPath === void 0) continue;
			if ((await this.ctx.fs.resolve(recordedPath, {
				cwd: repositoryRoot,
				signal
			})).targetKey !== target.targetKey) continue;
			if (fields.some((field) => field === "locked" || field.startsWith("locked "))) throw new Error(`worktree is locked: ${path}`);
			if (await this.ctx.fs.lstat(path, void 0, signal) !== void 0) throw new Error(`worktree path already exists: ${path}`);
			await run(repositoryRoot, [
				"worktree",
				"remove",
				"--",
				path
			]);
		}
		const branchRecreated = (await run(repositoryRoot, [
			"show-ref",
			"--verify",
			"--quiet",
			`refs/heads/${branch}`
		], [0, 1])).exitCode !== 0;
		try {
			await this.prepareDirectory(policy, repositoryRoot, this.ctx.fs.processPath(pool), path, signal);
			await run(repositoryRoot, [
				"--work-tree",
				repositoryRoot,
				"worktree",
				"add",
				"--no-checkout",
				...branchRecreated ? [
					"-b",
					branch,
					"--",
					path,
					baseCommit
				] : [
					"--",
					path,
					branch
				]
			]);
			const overrides = await filterOverrides(run, path);
			onPhase?.("checkout");
			await run(path, [
				"--work-tree",
				path,
				"reset",
				"--hard",
				"--no-recurse-submodules",
				"HEAD"
			], void 0, overrides);
			return {
				path,
				branchRecreated
			};
		} catch (error) {
			throw new Error(`Could not restore worktree ${branch} at ${path}. Any created checkout and branch are retained. ${error instanceof Error ? error.message : String(error)}`, { cause: error });
		}
	}
	/** Confine Git commands to the caller's sandbox policy, with hooks and the filesystem monitor disabled. */
	gitRunner(policy, signal) {
		return (directory, args, allowedExitCodes, env) => runCommand(this.ctx, this.config, policy, directory, [
			this.config.gitCommand,
			"-c",
			"core.hooksPath=/dev/null",
			"-c",
			"core.fsmonitor=false",
			...args
		], signal, allowedExitCodes, env);
	}
	/** Allocate the checkout directory exclusively inside the pool, creating a self-ignoring pool when it is new. */
	async prepareDirectory(policy, repositoryRoot, poolPath, path, signal) {
		await runCommand(this.ctx, this.config, policy, repositoryRoot, [
			this.config.nodeCommand,
			"--input-type=module",
			"-e",
			PREPARE_DIRECTORY,
			poolPath,
			path
		], signal);
	}
};
/**
* Build the environment that disables every configured clean, smudge, and
* process filter of the checkout for one command, without changing Git config.
* @param run - confined Git runner.
* @param path - checkout directory.
* @returns `GIT_CONFIG_*` overrides.
*/
async function filterOverrides(run, path) {
	const configured = await run(path, [
		"config",
		"--null",
		"--name-only",
		"--get-regexp",
		"^filter\\."
	], [0, 1]);
	const drivers = /* @__PURE__ */ new Set();
	for (const key of configured.stdout.split("\0")) {
		const separator = key.lastIndexOf(".");
		if (separator > 7 && [
			"clean",
			"smudge",
			"process",
			"required"
		].includes(key.slice(separator + 1))) drivers.add(key.slice(7, separator));
	}
	const overrides = { GIT_CONFIG_COUNT: String(drivers.size * 4) };
	let index = 0;
	for (const driver of drivers) for (const attribute of [
		"clean",
		"smudge",
		"process",
		"required"
	]) {
		overrides[`GIT_CONFIG_KEY_${index}`] = `filter.${driver}.${attribute}`;
		overrides[`GIT_CONFIG_VALUE_${index}`] = attribute === "required" ? "false" : "";
		index++;
	}
	return overrides;
}
//#endregion
export { WorktreeService, WorktreeService as default, generatedWorktreeName };
