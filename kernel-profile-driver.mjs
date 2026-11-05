// 内核启动 CPU profile 驱动：spawn 内核（--inspect=0）→ inspector 开始采样
// → 等 "dsh web:" → Profiler.stop → 落盘 .cpuprofile
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const repo = process.argv[2] ?? process.cwd();
const dshHome = process.argv[3];
const node = join(repo, "Kernel", "node.exe");
const bin = join(repo, "Kernel", "dsh", "lib", "bin.js");

const env = { ...process.env, DSH_HOME: dshHome, NODE_COMPILE_CACHE: join(dshHome, "kernel-compile-cache") };
const child = spawn(node, ["--inspect=0", bin, "web", "--no-open", "--port", "0"], { env });

let inspectUrl = null;
let bootStart = Date.now();
child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => {
  if (inspectUrl === null) {
    const m = chunk.match(/ws:\/\/[^\s]+/);
    if (m) inspectUrl = m[0];
  }
});
child.stdout.setEncoding("utf8");
let url = null;
child.stdout.on("data", (chunk) => {
  const m = chunk.match(/dsh web: (\S+)/);
  if (m && url === null) {
    url = m[1];
    console.log(`url at ${Date.now() - bootStart} ms`);
  }
});

// 等 inspector 就绪再连
while (inspectUrl === null) await new Promise((r) => setTimeout(r, 10));
const ws = new WebSocket(inspectUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });

let msgId = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
};
const send = (method, params = {}) => new Promise((resolve) => {
  const id = ++msgId;
  pending.set(id, resolve);
  ws.send(JSON.stringify({ id, method, params }));
});

await send("Profiler.enable");
await send("Profiler.setSamplingInterval", { interval: 500 });
await send("Profiler.start");
const t0 = Date.now();

while (url === null && Date.now() - t0 < 120000) await new Promise((r) => setTimeout(r, 20));
await new Promise((r) => setTimeout(r, 500)); // 让挂载收尾也进 profile
const stopped = await send("Profiler.stop");
const dir = process.argv[4] ?? process.cwd();
const file = join(dir, "kernel-boot.cpuprofile");
writeFileSync(file, JSON.stringify(stopped.result.profile));
console.log(`profile saved: ${file}`);
try { child.kill("SIGKILL"); } catch {}
process.exit(0);
