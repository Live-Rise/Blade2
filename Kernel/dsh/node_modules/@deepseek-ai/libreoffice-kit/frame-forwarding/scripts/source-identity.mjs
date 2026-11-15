/** Source identity for frame-forwarding inputs; generated engine receipts are not SDK sources. */
import { execFileSync } from 'node:child_process';

/** Read the selected commit and whether the SDK or owning package metadata has local changes. */
export function frameForwardingSourceIdentity(directory, { expectedCommit, environment = process.env, requireClean = false } = {}) {
  const git = args => execFileSync('git', args, { cwd: directory, encoding: 'utf8' }).trim();
  const sourceCommit = git(['rev-parse', 'HEAD']);
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)) throw new Error('Frame forwarding requires a complete source commit');
  if ((expectedCommit !== undefined && expectedCommit !== sourceCommit)
    || (environment.GITHUB_SHA && environment.GITHUB_SHA !== sourceCommit))
    throw new Error('Frame-forwarding selected source commit differs from HEAD');
  const sourceDirty = git(['status', '--porcelain', '--untracked-files=normal', '--', '.', '../entry/package.json']) !== '';
  if (requireClean && sourceDirty) throw new Error('Frame-forwarding release staging requires clean SDK sources and Kit metadata');
  return { sourceCommit, sourceDirty };
}
