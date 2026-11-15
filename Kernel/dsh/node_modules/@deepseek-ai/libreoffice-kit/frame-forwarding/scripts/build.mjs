/** Build precompiled receiver and producer SDK with the host C++ toolchain. */
import { mkdirSync, existsSync, writeFileSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { updateSdkMetadata, hashFile } from './metadata.mjs';
import { frameForwardingSourceIdentity } from './source-identity.mjs';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const sdk = updateSdkMetadata(root);
const { sourceCommit, sourceDirty } = frameForwardingSourceIdentity(root);
const archIndex = process.argv.indexOf('--arch');
const arch = archIndex < 0 ? process.arch : process.argv[archIndex + 1];
if (!['arm64', 'x64'].includes(arch) || !['darwin', 'win32'].includes(process.platform)) throw new Error('Unsupported frame-forwarding build architecture');
if (process.platform === 'win32' && arch !== process.arch && process.env.VSCMD_ARG_TGT_ARCH !== arch) throw new Error('Windows cross-build requires the matching MSVC developer environment');
const platform = `${process.platform}-${arch}`;
const output = resolve(root, 'prebuilds', platform);
mkdirSync(output, { recursive: true });
const nodeInclude = process.env.DSH_NODE_INCLUDE ?? ['/opt/homebrew/include/node', '/usr/local/include/node', '/usr/include/node'].find(path => existsSync(resolve(path, 'node_api.h')) && existsSync(resolve(path, 'uv.h')));
if (!nodeInclude || !existsSync(resolve(nodeInclude, 'node_api.h')) || !existsSync(resolve(nodeInclude, 'uv.h'))) throw new Error('Set DSH_NODE_INCLUDE to matching complete Node headers containing node_api.h and uv.h');
const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
};
if (process.platform === 'darwin') {
  const common = ['-arch', arch === 'x64' ? 'x86_64' : 'arm64', '-mmacosx-version-min=11.0', '-std=c++17', '-O2', '-fPIC', '-Iinclude', '-Isrc', '-Wall', '-Wextra'];
  const libs = ['-framework', 'IOSurface', '-framework', 'CoreFoundation', '-lbsm'];
  run('clang++', [...common, '-I' + nodeInclude, '-DNAPI_VERSION=8', '-DNODE_GYP_MODULE_NAME=dsh_frame_forwarding', '-shared', '-undefined', 'dynamic_lookup', '-Wl,-install_name,@rpath/frame-forwarding.node', 'src/receiver.cc', 'src/node-producer.cc', 'src/producer.cc', 'src/transport.cc', ...libs, '-o', resolve(output, 'frame-forwarding.node')]);
  run('codesign', ['--force', '--sign', '-', resolve(output, 'frame-forwarding.node')]);
  for (const source of ['producer', 'transport']) run('clang++', [...common, '-c', `src/${source}.cc`, '-o', resolve(output, `${source}.o`)]);
  run('ar', ['rcs', resolve(output, 'libdsh_frame_forwarding_sdk.a'), resolve(output, 'producer.o'), resolve(output, 'transport.o')]);
} else if (process.platform === 'win32') {
  const nodeLib = process.env.DSH_NODE_LIB;
  if (!nodeLib) throw new Error('Set DSH_NODE_LIB to the matching Node node.lib');
  const common = ['/nologo', '/std:c++17', '/O2', '/EHsc', '/MD', '/D_WIN32_WINNT=0x0602', '/Iinclude', '/Isrc'];
  run('cl', [...common, '/I' + nodeInclude, '/DNAPI_VERSION=8', '/DNODE_GYP_MODULE_NAME=dsh_frame_forwarding', '/LD', '/Fo' + output + '/', 'src/receiver.cc', 'src/node-producer.cc', 'src/producer.cc', 'src/transport.cc', 'src/win-delay-load.cc', '/link', '/DELAYLOAD:node.exe', 'delayimp.lib', '/OUT:' + resolve(output, 'frame-forwarding.node'), nodeLib, 'd3d11.lib', 'dxgi.lib', 'advapi32.lib', 'bcrypt.lib']);
  for (const source of ['producer', 'transport']) run('cl', [...common, '/c', `src/${source}.cc`, '/Fo' + resolve(output, `${source}.obj`)]);
  run('lib', ['/nologo', '/OUT:' + resolve(output, 'dsh_frame_forwarding_sdk.lib'), resolve(output, 'producer.obj'), resolve(output, 'transport.obj')]);
} else throw new Error(`Unsupported frame-forwarding platform ${platform}`);
const files = process.platform === 'darwin' ? ['frame-forwarding.node', 'libdsh_frame_forwarding_sdk.a'] : ['frame-forwarding.node', 'dsh_frame_forwarding_sdk.lib'];
const hashes = Object.fromEntries(files.map(file => [file, createHash('sha256').update(readFileSync(resolve(output, file))).digest('hex')]));
const sources = Object.fromEntries(sdk.buildSources.map(file => [file, hashFile(resolve(root, file))]));
writeFileSync(resolve(output, 'manifest.json'), JSON.stringify({ sources, sourceCommit, sourceDirty, version: sdk.version, protocolVersion: sdk.protocolVersion, capabilities: sdk.capabilities, platform, files: hashes }, null, 2) + '\n');
