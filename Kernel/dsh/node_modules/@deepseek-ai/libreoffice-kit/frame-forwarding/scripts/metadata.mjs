/** Shared source inventory and metadata for the frame-forwarding payload in LibreOffice Kit. */
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const sdkRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const targets = ['darwin-arm64', 'darwin-x64', 'win32-x64', 'win32-arm64'];
/**
 * Wire capabilities beyond protocol version 1 that every build of these sources implements and requires of its peer.
 * Version 1 builds without `transferable-tx` bind producers by PID, and builds without `producer-local-handles` send
 * Windows handles duplicated into the receiver; neither interoperates with these sources.
 */
export const capabilities = ['transferable-tx', 'producer-local-handles'];
export const buildSources = [
  'include/dsh_frame_forwarding.h', 'src/graphite-interop-internal.hxx', 'src/node-producer.cc', 'src/producer-internal.h', 'src/transport.h', 'src/transport.cc', 'src/relay-transport.inc',
  'src/producer.cc', 'src/receiver.cc', 'src/win-delay-load.cc', 'src/win-delay-load.LICENSE', 'src/graphite-interop.cc',
  'CMakeLists.txt', 'sdk.json', 'scripts/build.mjs', 'scripts/metadata.mjs', 'scripts/source-identity.mjs', 'scripts/build-webgpu.mjs', 'webgpu-dependencies.json',
];
export const webgpuSources = ['CMakeLists.txt', 'sdk.json', 'scripts/metadata.mjs', 'webgpu-dependencies.json',
  'scripts/build-webgpu.mjs', 'include/dsh_frame_forwarding.h', 'src/graphite-interop-internal.hxx', 'src/producer.cc', 'src/producer-internal.h', 'src/transport.cc', 'src/transport.h',
  'src/relay-transport.inc', 'src/graphite-interop.cc'];
export const presentationFiles = ['browser/index', 'browser/types', 'browser/painters', 'browser/display',
  'electron/index', 'electron/adapter', 'electron/transfer', 'electron/preload']
  .flatMap(file => [`lib/${file}.js`, `lib/${file}.d.ts`]);
export const payloadFiles = [
  ...presentationFiles,
  'index.mjs', 'index.d.ts', 'verify.mjs', 'verify.d.mts', 'receipt.json',
  'LICENSE', 'README.md', 'README.zh.md', ...buildSources,
  'scripts/verify-prebuilds.mjs',
];

export function hashFile(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}

/** Generate source metadata from the single version owned by the public Kit package. */
export function updateSdkMetadata(root = sdkRoot) {
  const kitFile = existsSync(join(root, '../package.json')) ? join(root, '../package.json') : join(root, '../entry/package.json');
  const kit = JSON.parse(readFileSync(kitFile, 'utf8'));
  const sdk = { version: kit.version, protocolVersion: 1, capabilities, header: 'include/dsh_frame_forwarding.h',
    sources: ['src/transport.cc', 'src/producer.cc'], cxxStandard: 17, platforms: targets,
    staticLibraries: Object.fromEntries(targets.map(target => [target,
      `prebuilds/${target}/${target.startsWith('win32-') ? 'dsh_frame_forwarding_sdk.lib' : 'libdsh_frame_forwarding_sdk.a'}`])),
    buildSources };
  writeFileSync(join(root, 'sdk.json'), JSON.stringify(sdk, null, 2) + '\n');
  writeFileSync(join(root, 'receipt.json'), JSON.stringify({ package: '@deepseek-ai/libreoffice-kit',
    version: kit.version, protocolVersion: 1, capabilities,
    files: Object.fromEntries(['LICENSE', ...webgpuSources].map(file => [file, hashFile(join(root, file))])) }, null, 2) + '\n');
  return sdk;
}
