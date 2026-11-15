/** Native Core patches included in every build and source receipt. */
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { root } from '../../scripts/platform-matrix.mjs';

/** Return ordered repository-relative native patch paths. */
export function corePatchFiles(repo = root) {
  const directory = 'engine/native/patches';
  return readdirSync(join(repo, directory)).filter(file => file.endsWith('.patch')).sort().map(file => `${directory}/${file}`);
}

/** Check the complete ordered patch result, including later patches that alter earlier context.
 * @param core - Disposable Core checkout at the pinned upstream revision.
 * @param repo - Source recipe root.
 * @returns Whether every patched path has the expected final bytes.
 */
export function hasNativePatchSet(core, repo = root) {
  const directory = mkdtempSync(join(tmpdir(), 'kit-patch-index-'));
  const env = { ...process.env, GIT_INDEX_FILE: join(directory, 'index') };
  const git = args => spawnSync('git', args, { cwd: core, env, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  const requireSuccess = args => {
    const result = git(args);
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`Cannot compute native patch result: ${result.stderr}`);
    return result.stdout;
  };
  try {
    requireSuccess(['read-tree', 'HEAD']);
    for (const file of corePatchFiles(repo)) requireSuccess(['apply', '--cached', '--whitespace=nowarn', join(repo, file)]);
    const paths = requireSuccess(['diff', '--cached', '--name-only', '-z', 'HEAD']).split('\0').filter(Boolean);
    if (!paths.length) return true;
    const result = git(['diff', '--quiet', '--', ...paths]);
    if (result.error) throw result.error;
    if (result.status !== 0 && result.status !== 1) throw new Error(`Cannot inspect native patch result: ${result.stderr}`);
    return result.status === 0;
  } finally { rmSync(directory, { recursive: true, force: true }); }
}
