/** Validate all four frame-forwarding payloads against their public Kit package version. */
import { readFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { sdkRoot } from './metadata.mjs';
import { verifyFrameForwardingPrebuilds } from '../verify.mjs';

export async function verifyPrebuilds(directory = sdkRoot, sourceCommit) {
  const kitFile = existsSync(join(directory, '../package.json')) ? join(directory, '../package.json') : join(directory, '../entry/package.json');
  const kit = JSON.parse(readFileSync(kitFile, 'utf8'));
  return verifyFrameForwardingPrebuilds(directory, { version: kit.version, sourceCommit });
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url)
  console.log(JSON.stringify(await verifyPrebuilds(process.argv[2] && resolve(process.argv[2]), process.argv[3])));
