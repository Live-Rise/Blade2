/** Verify the frame-forwarding sources compiled into the LibreOffice engine. */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join, posix } from 'node:path';
export function frameForwardingSdkFiles(root) {
  const directory = 'packages/frame-forwarding';
  const receipt = JSON.parse(readFileSync(join(root, directory, 'receipt.json'), 'utf8'));
  const sdk = JSON.parse(readFileSync(join(root, directory, 'sdk.json'), 'utf8'));
  if (receipt.package !== '@deepseek-ai/libreoffice-kit' || receipt.version !== sdk.version || receipt.protocolVersion !== sdk.protocolVersion
    || !Array.isArray(sdk.capabilities) || !['transferable-tx', 'producer-local-handles'].every(capability => sdk.capabilities.includes(capability))
    || JSON.stringify(receipt.capabilities) !== JSON.stringify(sdk.capabilities))
    throw new Error('Unsupported frame-forwarding SDK source receipt');
  const files = Object.keys(receipt.files);
  for (const file of files) {
    if (!/^[a-zA-Z0-9_.-]+(?:\/[a-zA-Z0-9_.-]+)*$/.test(file) || file.split('/').some(part => part === '.' || part === '..')) throw new Error('Invalid frame-forwarding SDK receipt path');
    const bytes = readFileSync(join(root, directory, file));
    if (createHash('sha256').update(bytes).digest('hex') !== receipt.files[file]) throw new Error(`Frame-forwarding SDK source hash differs: ${file}`);
  }
  return [`${directory}/receipt.json`, ...files.map(file => `${directory}/${file}`)];
}

/** Require the complete current SDK recipe; LICENSE is recorded as an installed artifact. */
export function verifyFrameForwardingSdkBuildSources(root, receipt) {
  if (!receipt.sources || Object.keys(receipt.sources).length === 0) throw new Error('WebGPU build has no SDK source hashes');
  const directory = join(root, 'packages/frame-forwarding');
  const source = JSON.parse(readFileSync(join(directory, 'receipt.json'), 'utf8'));
  for (const file of Object.keys(source.files)) {
    if (file !== 'LICENSE' && !Object.hasOwn(receipt.sources, file))
      throw new Error(`WebGPU build omits SDK source: ${file}`);
  }
  for (const [file, hash] of Object.entries(receipt.sources)) {
    if (file === 'LICENSE' || !Object.hasOwn(source.files, file) || source.files[file] !== hash)
      throw new Error(`WebGPU build SDK source differs: ${file}`);
  }
}

/** Keep the SDK recipe and hashes while removing host-specific tool/cache paths. */
export function publicFrameForwardingSdkBuildReceipt(receipt) {
  const configure = receipt.configure;
  if (!Array.isArray(configure)) throw new Error('WebGPU build has no CMake recipe');
  const argument = flag => {
    const index = configure.indexOf(flag);
    return index < 0 ? undefined : configure[index + 1];
  };
  const definition = name => configure.find(value => value.startsWith(`-D${name}=`))?.slice(name.length + 3);
  const build = argument('-B');
  const mappings = [
    [argument('-S'), '/dsh-sdk'],
    [build && posix.dirname(build.replaceAll('\\', '/')), '/dsh-build'],
    [definition('DSH_DAWN_SOURCE_DIR'), '/dawn'],
    [definition('CMAKE_INSTALL_PREFIX'), '/dsh-install'],
    [definition('Python3_EXECUTABLE'), 'python3'],
  ];
  if (mappings.some(([path]) => typeof path !== 'string' || path.length === 0))
    throw new Error('WebGPU CMake recipe omits source, build, install or Python paths');
  mappings.sort(([left], [right]) => right.length - left.length);
  return { ...receipt, configure: configure.map(value => {
    for (const [path, replacement] of mappings)
      value = value.replaceAll(path, replacement).replaceAll(path.replaceAll('\\', '/'), replacement)
        .replaceAll(path.replaceAll('/', '\\'), replacement);
    return value;
  }) };
}
