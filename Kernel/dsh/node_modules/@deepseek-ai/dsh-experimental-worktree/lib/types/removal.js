/**
 * Confirm that a recorded checkout names a canonical directory inside the
 * configured pool of the source checkout it claims, and that the source is
 * still that repository's top level.
 * @param ctx - execution-world filesystem provider.
 * @param directory - configured relative worktree pool.
 * @param request - saved checkout identity.
 * @param run - bounded Git runner with the owner's policy.
 * @param signal - cancellation of the lookups.
 * @returns the resolved root and target, or null when any identity check fails.
 */
export async function checkoutIdentity(ctx, directory, request, run, signal) {
    const { path, repositoryRoot } = request;
    const root = await ctx.fs.resolve(repositoryRoot, { signal });
    const pool = await ctx.fs.resolve(directory, { cwd: repositoryRoot, signal });
    const target = await ctx.fs.resolve(path, { cwd: repositoryRoot, signal });
    if (ctx.fs.processPath(root) !== repositoryRoot || ctx.fs.processPath(target) !== path
        || !ctx.fs.contains(root, pool) || !ctx.fs.contains(pool, target) || pool.targetKey === target.targetKey)
        return null;
    const top = await run(repositoryRoot, ['rev-parse', '--show-toplevel']);
    const actualRoot = await ctx.fs.resolve(top.stdout.replace(/\r?\n$/, ''), { cwd: repositoryRoot, signal });
    if (actualRoot.targetKey !== root.targetKey)
        return null;
    return { root, target };
}
/**
 * Find the registration of `target` among the source repository's worktrees.
 * The listing grows with the number of worktrees; it is streamed without an
 * output limit, and its entries are kept in memory while `target` is resolved.
 * @param ctx - execution-world filesystem provider.
 * @param request - saved checkout identity.
 * @param target - resolved checkout directory.
 * @param scan - streaming Git runner with the owner's policy.
 * @param signal - cancellation of the lookups.
 * @returns the porcelain fields of the matching entry, or undefined when none is registered.
 */
export async function registration(ctx, request, target, scan, signal) {
    const entries = [];
    // Each entry of the -z porcelain listing starts with its worktree line and ends with an empty field.
    let fields = [];
    await scan(request.repositoryRoot, ['worktree', 'list', '--porcelain', '-z'], (field) => {
        if (field.startsWith('worktree ')) {
            fields = [field];
            entries.push({ path: field.slice('worktree '.length), fields });
        }
        else if (field.length > 0)
            fields.push(field);
        return true;
    });
    for (const entry of entries) {
        const recorded = await ctx.fs.resolve(entry.path, { cwd: request.repositoryRoot, signal });
        if (recorded.targetKey === target.targetKey)
            return entry.fields;
    }
    return undefined;
}
/**
 * Inspect the checkout and, when eligible and still admitted after inspection,
 * remove it and its registration with a non-force `git worktree remove`. The
 * branch is never deleted; a rejection can leave a partial directory.
 * @param ctx - execution-world filesystem provider.
 * @param directory - configured relative worktree pool.
 * @param request - saved checkout identity.
 * @param git - confined Git runners with the owner's policy.
 * @param signal - cancellation before or during removal.
 * @param admit - synchronous final admission after inspection; false retains the checkout.
 * @returns true when removed; false when inspection or admission retains it; Git failures reject.
 */
export async function removeCheckout(ctx, directory, request, git, signal, admit) {
    if (!await inspectRemoval(ctx, directory, request, git, signal))
        return false;
    signal.throwIfAborted();
    if (!admit())
        return false;
    await git.read(request.repositoryRoot, ['worktree', 'remove', '--', request.path]);
    return true;
}
/**
 * Verify checkout identity and that `git status` reports no tracked changes or
 * untracked files; its output is read only up to the first entry. Ignored files
 * are not counted, nor are edits to skip-worktree or assume-unchanged files,
 * which `git status` hides; `git worktree remove` discards both. A checkout
 * whose administrative directory holds submodule repositories is ineligible,
 * as Git refuses to remove it; a committed embedded repository is not
 * inspected, so Git's refusal of that removal rejects.
 * @param ctx - execution-world filesystem provider.
 * @param directory - configured relative worktree pool.
 * @param request - saved checkout identity.
 * @param git - confined Git runners with the owner's policy.
 * @param signal - cancellation of inspection.
 * @returns false for an ineligible checkout; inspection failures reject.
 */
export async function inspectRemoval(ctx, directory, request, git, signal) {
    const { path, branch } = request;
    const identity = await checkoutIdentity(ctx, directory, request, git.read, signal);
    if (identity === null)
        return false;
    if ((await ctx.fs.lstat(path, undefined, signal))?.type !== 'directory')
        return false;
    const fields = await registration(ctx, request, identity.target, git.scan, signal);
    // The primary registration is excluded even when the configured pool contains it.
    if (fields === undefined || !fields.includes(`branch refs/heads/${branch}`)
        || fields.some(field => field === 'detached' || field.startsWith('locked') || field.startsWith('prunable')))
        return false;
    const head = await git.read(path, ['symbolic-ref', 'HEAD']);
    if (head.stdout.trim() !== `refs/heads/${branch}`)
        return false;
    const changed = await git.scan(path, ['status', '--porcelain=v1', '-z', '--untracked-files=all', '--ignore-submodules=none'], () => false);
    if (changed)
        return false;
    // Git refuses removal whenever this directory exists, populated or not.
    const modules = await git.read(path, ['rev-parse', '--git-path', 'modules']);
    return await ctx.fs.lstat(modules.stdout.replace(/\r?\n$/, ''), { cwd: path }, signal) === undefined;
}
//# sourceMappingURL=removal.js.map