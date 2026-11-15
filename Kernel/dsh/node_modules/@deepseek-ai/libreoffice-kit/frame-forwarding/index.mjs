/** Electron-independent Node-API producer and receiver for native rendering. */
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
let binding;
const budgets = new WeakMap();
function loadBinding() {
  return binding ??= require(fileURLToPath(new URL(`./prebuilds/${process.platform}-${process.arch}/frame-forwarding.node`, import.meta.url)));
}
/** Share one allocation budget across all native views in a window. */
export function createBudget(maxBytes) {
  const budget = Object.freeze({});
  budgets.set(budget, loadBinding().createBudget(maxBytes));
  return budget;
}
/**
 * Create a frame-forwarding channel and publish its listener. Calls onEvent only on the JavaScript thread. The returned
 * TX credential lets any producer holding it connect; the channel serves one connection at a time.
 */
export function createReceiver({ maxFrames, maxBytes, budget, displayAdapter, readbackTimeoutMs, framePolicy = 'ordered' }, onEvent) {
  loadBinding();
  const allocationBudget = budgets.get(budget);
  if (!allocationBudget) throw new TypeError("Receiver requires a budget from createBudget()");
  if (framePolicy !== 'ordered' && framePolicy !== 'latest') throw new TypeError('Invalid native frame policy');
  const { native, tx } = binding.create(maxFrames, maxBytes, onEvent, allocationBudget, displayAdapter === null ? '' : displayAdapter,
    readbackTimeoutMs, framePolicy === 'latest');
  let closed;
  return {
    tx,
    send(connectionId, json) { binding.send(native, connectionId, json); },
    release(deliveryId) { return binding.release(native, deliveryId); },
    close() { return closed ??= binding.close(native); },
  };
}
/** Identify the adapter owning a native texture, or null when unavailable. */
export function getAdapter(handle) {
  return loadBinding().getAdapter(handle) || null;
}

/** Create a producer whose transport and surface operations run on one native thread. */
export function createProducer({ tx, maxPendingOperations, operationTimeoutMs }, onEvent) {
  const api = loadBinding();
  const { native, ready, closed } = api.createProducer(tx, maxPendingOperations, operationTimeoutMs, onEvent);
  const command = (kind, ...args) => api.producerCommand(native, kind, ...args);
  return Object.freeze({
    ready,
    closed,
    async acquire(width, height) {
      const surface = await command(0, width, height);
      const discard = async () => command(3, surface);
      return Object.freeze({
        width, height,
        async writeBgra(pixels, stride) {
          if (!Buffer.isBuffer(pixels) || pixels.buffer instanceof SharedArrayBuffer) {
            throw new TypeError('CPU pixels require a Buffer backed by a non-shared ArrayBuffer');
          }
          await command(1, surface, pixels, stride);
        },
        async submit(frameId, metadata) { await command(2, surface, frameId, metadata); },
        async submitLatest(frameId, metadata) { await command(5, surface, frameId, metadata); },
        discard,
        close: discard,
      });
    },
    async send(json) { await command(4, json); },
    close() { api.closeProducer(native); return closed; },
  });
}
