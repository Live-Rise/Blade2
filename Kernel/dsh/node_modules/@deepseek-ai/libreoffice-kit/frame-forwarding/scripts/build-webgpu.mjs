/** Build the producer adapter with unmodified, hash-pinned Dawn and its required dependencies. */
import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream } from 'node:fs';
import { cp, mkdir, readFile, readdir, readlink, rename, rm, stat, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { availableParallelism } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { webgpuSources } from './metadata.mjs';

const defaultRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
async function exists(path) {
  try { await stat(path); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}
async function fileHash(path) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(path), hash);
  return hash.digest('hex');
}
function run(command, args, cwd, capture = false) {
  const result = spawnSync(command, args, { cwd, env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
    stdio: capture ? 'pipe' : 'inherit', encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status}: ${result.stderr ?? ''}`);
  return result.stdout?.trim();
}
async function tree(directory, prefix = '') {
  const result = {};
  for (const entry of (await readdir(join(directory, prefix), { withFileTypes: true })).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)) {
    if (entry.name === '.git') continue;
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    const path = join(directory, relative);
    if (entry.isDirectory()) Object.assign(result, await tree(directory, relative));
    else if (entry.isSymbolicLink()) result[relative] = `link:${await readlink(path)}`;
    else if (entry.isFile()) result[relative] = await fileHash(path);
    else throw new Error(`Unsupported source entry: ${path}`);
  }
  return result;
}
function verifyTree(actual, expected, name) {
  const changed = [...new Set([...Object.keys(actual), ...Object.keys(expected)])]
    .filter(path => actual[path] !== expected[path]);
  if (changed.length) throw new Error(`${name} differs from its pinned archive: ${changed.slice(0, 10).join(', ')}`);
}
/** Hash every source input in the native WebGPU SDK build recipe. */
export async function webgpuBuildSources(root = defaultRoot) {
  const sources = {};
  for (const file of webgpuSources)
    sources[file] = await fileHash(join(root, file));
  return sources;
}
async function prepare(name, dependency, cache) {
  const base = join(cache, `${name}-${dependency.revision}`);
  const directory = join(base, name);
  const archive = `${base}.tar.gz`;
  if (!(await exists(archive))) {
    if (dependency.sourceTreeSha256) {
      const repository = `${base}.git`;
      await mkdir(repository, { recursive: true });
      try {
        run('git', ['init', '--bare'], repository);
        run('git', ['fetch', '--depth=1', dependency.url, dependency.revision], repository);
        run('git', ['-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'archive', '--format=tar.gz',
          `--output=${archive}.download`, 'FETCH_HEAD'], repository);
      } finally { await rm(repository, { recursive: true, force: true }); }
    } else {
      const response = await fetch(dependency.url);
      if (!response.ok || !response.body) throw new Error(`Download ${name} failed: ${response.status}`);
      await pipeline(response.body, createWriteStream(`${archive}.download`));
    }
    if (dependency.sha256 && await fileHash(`${archive}.download`) !== dependency.sha256) throw new Error(`${name} archive SHA-256 mismatch`);
    await rename(`${archive}.download`, archive);
  }
  if (dependency.sha256 && await fileHash(archive) !== dependency.sha256) throw new Error(`${name} archive SHA-256 mismatch`);
  // Re-extract the verified archive so a modified cached receipt cannot authorize modified sources.
  await rm(base, { recursive: true, force: true });
  await mkdir(directory, { recursive: true });
  run('tar', ['-xzf', archive.replaceAll('\\', '/'), '-C', directory.replaceAll('\\', '/'), `--strip-components=${dependency.stripComponents}`], cache);
  const expected = await tree(directory);
  // Git archives are verified by complete file contents independently of archive metadata.
  if (dependency.sourceTreeSha256 && sha256(JSON.stringify(expected)) !== dependency.sourceTreeSha256)
    throw new Error(`${name} source tree SHA-256 mismatch`);
  return { path: directory, files: expected, sha256: sha256(JSON.stringify(expected)) };
}

/**
 * Build and install a self-contained native WebGPU SDK prefix.
 * @param options - SDK source root, architecture, output/cache directories, optional pristine Dawn source and bounded parallelism.
 * @returns Installed prefix; receipt.json identifies source archives, build configuration and installed bytes.
 */
export async function buildWebGPU({ root = defaultRoot, arch = process.arch, output, cache, source = process.env.DSH_DAWN_SOURCE,
  jobs = Math.min(4, availableParallelism()), prepareOnly = false } = {}) {
  root = resolve(root);
  if (!['darwin', 'win32'].includes(process.platform) || !['arm64', 'x64'].includes(arch)) throw new Error('WebGPU SDK requires macOS or Windows x64/ARM64');
  if (process.platform === 'win32' && process.env.VSCMD_ARG_TGT_ARCH !== arch)
    throw new Error(`Initialize the MSVC developer environment for ${arch}`);
  if (!Number.isSafeInteger(jobs) || jobs < 1) throw new Error('Build jobs must be a positive integer');
  const platform = `${process.platform}-${arch}`;
  output = resolve(output ?? join(root, 'prebuilds', platform, 'webgpu'));
  cache = resolve(cache ?? join(root, '.build', 'webgpu'));
  const dependencyCache = join(cache, 'dependencies');
  await mkdir(dependencyCache, { recursive: true });
  const manifestBytes = await readFile(join(root, 'webgpu-dependencies.json'));
  const { dependencies } = JSON.parse(manifestBytes);
  const results = await Promise.allSettled(Object.entries(dependencies).map(async ([name, dependency]) =>
    [name, await prepare(name, dependency, dependencyCache)]));
  for (const result of results) if (result.status === 'rejected') throw result.reason;
  const prepared = Object.fromEntries(results.map(result => result.value));
  let dawn = prepared.dawn.path;
  if (source) {
    dawn = resolve(source);
    verifyTree(await tree(dawn), prepared.dawn.files, 'DSH_DAWN_SOURCE');
  }
  for (const [name, dependency] of Object.entries(dependencies)) {
    if (name !== 'dawn' && !(await readFile(join(dawn, 'DEPS'), 'utf8')).includes(dependency.revision))
      throw new Error(`${name} revision is absent from the pinned Dawn DEPS`);
  }
  if (prepareOnly) return { source: dawn, dependencies: prepared, output };
  const cmake = process.env.DSH_CMAKE ?? 'cmake';
  const python = run(process.env.DSH_PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3'),
    ['-c', 'import sys; print(sys.executable)'], root, true);
  const build = join(cache, process.platform === 'win32' ? `${platform}-clang` : platform);
  const pathFlags = [[root, '/dsh-sdk'], [cache, '/dsh-build'], [dawn, '/dawn']]
    .flatMap(([from, to]) => ['file', 'debug'].map(kind =>
      `${process.platform === 'win32' ? '/clang:' : ''}-f${kind}-prefix-map="${from.replaceAll('\\', '/')}=${to}"`)).join(' ');
  const off = [
    'DAWN_FETCH_DEPENDENCIES', 'DAWN_BUILD_SAMPLES', 'DAWN_BUILD_TESTS', 'DAWN_BUILD_NODE_BINDINGS',
    'DAWN_BUILD_BENCHMARKS', 'DAWN_BUILD_FUZZERS', 'DAWN_BUILD_PROTOBUF', 'DAWN_ENABLE_NULL',
    'DAWN_ENABLE_D3D11', 'DAWN_ENABLE_DESKTOP_GL', 'DAWN_ENABLE_OPENGLES', 'DAWN_ENABLE_VULKAN',
    'DAWN_ENABLE_SPIRV_VALIDATION', 'DAWN_ENABLE_WEBGPU_ON_WEBGPU', 'DAWN_USE_GLFW', 'DAWN_USE_X11',
    'DAWN_USE_WAYLAND', 'DAWN_USE_WINDOWS_UI', 'DAWN_USE_BUILT_DXC', 'TINT_BUILD_CMD_TOOLS',
    'TINT_BUILD_TESTS', 'TINT_BUILD_BENCHMARKS', 'TINT_BUILD_FUZZERS', 'TINT_BUILD_IR_BINARY',
    'TINT_BUILD_SPV_READER', 'TINT_BUILD_SPV_WRITER', 'TINT_BUILD_GLSL_WRITER', 'TINT_BUILD_GLSL_VALIDATOR',
    'TINT_BUILD_NULL_WRITER',
  ];
  const definitions = {
    DSH_FRAME_FORWARDING_WEBGPU: 'ON', DSH_DAWN_SOURCE_DIR: dawn, CMAKE_BUILD_TYPE: 'Release',
    CMAKE_INSTALL_PREFIX: output, CMAKE_INSTALL_LIBDIR: 'lib', CMAKE_POSITION_INDEPENDENT_CODE: 'ON',
    CMAKE_MSVC_RUNTIME_LIBRARY: 'MultiThreadedDLL', Python3_EXECUTABLE: python,
    DAWN_ENABLE_PIC: 'ON', DAWN_FORCE_SYSTEM_COMPONENT_LOAD: 'ON',
    DAWN_ENABLE_METAL: process.platform === 'darwin' ? 'ON' : 'OFF',
    DAWN_ENABLE_D3D12: process.platform === 'win32' ? 'ON' : 'OFF',
    TINT_BUILD_MSL_WRITER: process.platform === 'darwin' ? 'ON' : 'OFF',
    TINT_BUILD_HLSL_WRITER: process.platform === 'win32' ? 'ON' : 'OFF',
    DAWN_ABSEIL_DIR: prepared['abseil-cpp'].path, DAWN_SPIRV_HEADERS_DIR: prepared['spirv-headers'].path,
    DAWN_JINJA2_DIR: prepared.jinja2.path, DAWN_MARKUPSAFE_DIR: prepared.markupsafe.path,
    ...(process.platform === 'win32' ? {
      CMAKE_C_COMPILER: 'clang-cl', CMAKE_CXX_COMPILER: 'clang-cl',
      CMAKE_C_COMPILER_TARGET: arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc',
      CMAKE_CXX_COMPILER_TARGET: arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc',
      CMAKE_SYSTEM_NAME: 'Windows', CMAKE_SYSTEM_PROCESSOR: arch === 'arm64' ? 'ARM64' : 'AMD64',
      CMAKE_C_FLAGS: pathFlags, CMAKE_CXX_FLAGS: pathFlags, CMAKE_EXE_LINKER_FLAGS: '', CMAKE_SHARED_LINKER_FLAGS: '',
    } : {}),
    ...(process.platform === 'darwin' ? { CMAKE_C_FLAGS: pathFlags, CMAKE_CXX_FLAGS: pathFlags, CMAKE_OBJCXX_FLAGS: pathFlags, CMAKE_OSX_ARCHITECTURES: arch === 'x64' ? 'x86_64' : 'arm64', CMAKE_OSX_DEPLOYMENT_TARGET: '11.0' } : {}),
  };
  const configure = ['-S', root, '-B', build, '-G', 'Ninja', ...off.map(key => `-D${key}=OFF`),
    ...Object.entries(definitions).map(([key, value]) => `-D${key}=${value}`)];
  const sources = await webgpuBuildSources(root);
  const start = performance.now();
  run(cmake, configure, root);
  run(cmake, ['--build', build, '--config', 'Release', '--target', 'dsh_webgpu', '--parallel', String(jobs)], root);
  run(cmake, ['--install', build, '--config', 'Release'], root);
  for (const [name, dependency] of Object.entries(dependencies)) {
    for (const license of dependency.licenses) {
      const destination = join(output, 'licenses', name, license);
      await mkdir(dirname(destination), { recursive: true });
      await cp(join(prepared[name].path, license), destination, { recursive: true });
    }
  }
  await cp(join(root, 'LICENSE'), join(output, 'licenses', 'DSH-LICENSE'));
  await rm(join(output, 'include/dsh_webgpu.h'), { force: true });
  const artifacts = await tree(output);
  delete artifacts['receipt.json'];
  for (const [file, hash] of Object.entries(sources))
    if (await fileHash(join(root, file)) !== hash) throw new Error(`SDK source changed during build: ${file}`);
  const receipt = { schemaVersion: 1, platform, sourceManifestSha256: sha256(manifestBytes), sources,
    dependencies: Object.fromEntries(Object.entries(dependencies).map(([name, dependency]) =>
      [name, { ...dependency, sourceTreeSha256: prepared[name].sha256 }])),
    configure, jobs, elapsedSeconds: (performance.now() - start) / 1000,
    cmake: run(cmake, ['--version'], root, true), artifacts };
  await writeFile(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
  return output;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  const options = {};
  for (let index = 2; index < process.argv.length; ++index) {
    const flag = process.argv[index];
    if (flag === '--prepare-only') { options.prepareOnly = true; continue; }
    const key = { '--root': 'root', '--output': 'output', '--cache': 'cache', '--arch': 'arch', '--jobs': 'jobs', '--source': 'source' }[flag];
    if (!key || !process.argv[index + 1] || process.argv[index + 1].startsWith('--')) throw new Error(`Invalid build argument: ${flag}`);
    options[key] = key === 'jobs' ? Number(process.argv[++index]) : process.argv[++index];
  }
  const result = await buildWebGPU(options);
  console.log(JSON.stringify(options.prepareOnly ? { source: result.source, output: result.output } : { output: result }));
}
