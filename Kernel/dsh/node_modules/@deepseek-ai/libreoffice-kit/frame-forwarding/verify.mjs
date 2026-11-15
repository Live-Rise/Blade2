/** Verify complete frame-forwarding SDK payloads before installation, packaging, or signing. */
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { buildSources, capabilities, targets } from './scripts/metadata.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const sameCapabilities = value => JSON.stringify(value) === JSON.stringify(capabilities);
function verifyObject(bytes, target, subject, archive = false) {
  if (target.platform === 'win32') {
    let machine;
    if (archive) {
      if (bytes.length < 20) throw new Error(`${subject} is not a Windows object`);
      machine = bytes.readUInt16LE(0) === 0 && bytes.readUInt16LE(2) === 0xffff ? bytes.readUInt16LE(6) : bytes.readUInt16LE(0);
    } else {
      if (bytes.length < 64 || bytes.readUInt16LE(0) !== 0x5a4d) throw new Error(`${subject} is not a Windows binary`);
      const pe = bytes.readUInt32LE(0x3c);
      if (pe > bytes.length - 6 || bytes.readUInt32LE(pe) !== 0x4550) throw new Error(`${subject} has an invalid PE header`);
      machine = bytes.readUInt16LE(pe + 4);
    }
    if (machine !== (target.arch === 'arm64' ? 0xaa64 : 0x8664)) throw new Error(`${subject} is not Windows ${target.arch}`);
  } else if (bytes.length < 8 || bytes.readUInt32LE(0) !== 0xfeedfacf || bytes.readUInt32LE(4) !== (target.arch === 'arm64' ? 0x100000c : 0x1000007)) {
    throw new Error(`${subject} is not macOS ${target.arch}`);
  }
}
function verifyArchive(bytes, target, subject) {
  if (bytes.subarray(0, 8).toString() !== '!<arch>\n') throw new Error(`${subject} is not a static archive`);
  let offset = 8, objects = 0;
  while (offset < bytes.length) {
    if (offset + 60 > bytes.length || bytes.subarray(offset + 58, offset + 60).toString() !== '`\n') throw new Error(`${subject} has an invalid archive member`);
    let name = bytes.subarray(offset, offset + 16).toString().trim();
    const sizeText = bytes.subarray(offset + 48, offset + 58).toString().trim();
    const size = Number(sizeText), start = offset + 60;
    if (!/^\d+$/.test(sizeText) || !Number.isSafeInteger(size) || start + size > bytes.length) throw new Error(`${subject} has an invalid archive size`);
    let contents = bytes.subarray(start, start + size);
    if (name.startsWith('#1/')) {
      const length = Number(name.slice(3));
      if (!Number.isSafeInteger(length) || length <= 0 || length > size) throw new Error(`${subject} has an invalid archive name`);
      name = contents.subarray(0, length).toString().replace(/\0+$/, '');
      contents = contents.subarray(length);
    }
    if (!['/', '//', '/SYM64/'].includes(name) && !name.startsWith('__.SYMDEF')) {
      verifyObject(contents, target, `${subject}:${name}`, true);
      objects++;
    }
    offset = start + size + size % 2;
  }
  if (!objects || offset !== bytes.length) throw new Error(`${subject} has no complete object payload`);
}

/**
 * Check one target's addon, static SDK, and source files against its build receipt.
 * @param {string} directory Frame-forwarding SDK directory.
 * @param {{platform:string,arch:string}} target Operating system and CPU architecture.
 * @returns {Promise<void>} Resolves only for a complete matching SDK target.
 */
export async function verifyFrameForwardingPackage(directory, target) {
  const name = `${target.platform}-${target.arch}`;
  if (!targets.includes(name)) throw new Error(`Frame forwarding has no packaged backend for ${name}`);
  const root = join(directory, 'prebuilds', name);
  const manifest = JSON.parse(await readFile(join(root, 'manifest.json'), 'utf8'));
  const sdk = JSON.parse(await readFile(join(directory, 'sdk.json'), 'utf8'));
  if (typeof sdk.version !== 'string' || !sdk.version || sdk.protocolVersion !== 1
    || !sameCapabilities(sdk.capabilities) || !sameCapabilities(manifest.capabilities)
    || JSON.stringify(sdk.platforms) !== JSON.stringify(targets)
    || JSON.stringify(sdk.buildSources) !== JSON.stringify(buildSources)
    || manifest.platform !== name || manifest.protocolVersion !== sdk.protocolVersion || manifest.version !== sdk.version
    || !/^[a-f0-9]{40}$/.test(manifest.sourceCommit) || typeof manifest.sourceDirty !== 'boolean') throw new Error(`Frame-forwarding build receipt does not match ${name}`);
  const library = target.platform === 'win32' ? 'dsh_frame_forwarding_sdk.lib' : 'libdsh_frame_forwarding_sdk.a';
  if (sdk.staticLibraries?.[name] !== `prebuilds/${name}/${library}`) throw new Error(`Frame-forwarding library metadata differs: ${name}`);
  if (Object.keys(manifest.files ?? {}).sort().join(',') !== [library, 'frame-forwarding.node'].sort().join(',')) throw new Error(`Frame-forwarding artifact inventory differs: ${name}`);
  for (const filename of ['frame-forwarding.node', library]) {
    const bytes = await readFile(join(root, filename));
    if (digest(bytes) !== manifest.files[filename]) throw new Error(`Frame-forwarding artifact hash mismatch: ${name}/${filename}`);
    if (filename === 'frame-forwarding.node') verifyObject(bytes, target, 'Frame-forwarding addon');
    else verifyArchive(bytes, target, 'Frame-forwarding producer SDK');
  }
  if (Object.keys(manifest.sources ?? {}).sort().join(',') !== [...buildSources].sort().join(',')) throw new Error(`Frame-forwarding source inventory differs: ${name}`);
  for (const source of buildSources) {
    if (digest(await readFile(join(directory, source))) !== manifest.sources[source]) throw new Error(`Frame-forwarding source receipt differs: ${name}/${source}`);
  }
}

/**
 * Require all four payloads to share the expected package version and source commit.
 * @param {string} directory Frame-forwarding SDK directory.
 * @param {{version?:string,sourceCommit?:string}} expected Release identity, when known by the caller.
 * @returns {Promise<{version:string,protocolVersion:number,capabilities:string[],sourceCommit:string}>} Shared build identity.
 */
export async function verifyFrameForwardingPrebuilds(directory, expected = {}) {
  const sdk = JSON.parse(await readFile(join(directory, 'sdk.json'), 'utf8'));
  if (expected.version !== undefined && sdk.version !== expected.version) throw new Error('Frame-forwarding version differs from the Kit package');
  let sourceCommit = expected.sourceCommit;
  for (const name of targets) {
    const [platform, arch] = name.split('-');
    await verifyFrameForwardingPackage(directory, { platform, arch });
    const manifest = JSON.parse(await readFile(join(directory, 'prebuilds', name, 'manifest.json'), 'utf8'));
    if (manifest.sourceDirty) throw new Error(`Frame-forwarding artifact was built from dirty SDK sources: ${name}`);
    sourceCommit ??= manifest.sourceCommit;
    if (sourceCommit !== manifest.sourceCommit) throw new Error(`Frame-forwarding source commit differs: ${name}`);
  }
  return { version: sdk.version, protocolVersion: sdk.protocolVersion, capabilities: sdk.capabilities, sourceCommit };
}
