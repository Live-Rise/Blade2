/** Identify Chromium's Windows compositor adapter from an actual exported texture. */
import { app, BrowserWindow } from 'electron';
/**
 * Await Chromium graphics initialization and identify the adapter within one deadline.
 * @param getAdapter - Native DXGI handle inspection.
 * @param timeoutMs - Total graphics initialization and detection deadline.
 * @param signal - Cancels detection when the owning Host or window closes.
 * @returns Chromium's adapter, or null when shared texture compatibility cannot be established.
 */
export async function detectFrameForwardingAdapter(getAdapter, timeoutMs, signal) {
    if (process.platform !== 'win32' || signal.aborted)
        return null;
    const lifetime = new AbortController();
    const abort = () => { lifetime.abort(); };
    const timer = setTimeout(abort, timeoutMs);
    signal.addEventListener('abort', abort, { once: true });
    if (signal.aborted)
        abort();
    try {
        if (!await graphicsReady(lifetime.signal))
            return null;
        for (let attempt = 0; attempt < 2 && !lifetime.signal.aborted; attempt++) {
            const adapter = await probeFrameForwardingAdapter(getAdapter, timeoutMs / 2, lifetime.signal);
            if (adapter !== null)
                return adapter;
        }
        return null;
    }
    finally {
        clearTimeout(timer);
        signal.removeEventListener('abort', abort);
    }
}
/** Feature status is usable after complete GPU information has been collected. */
function graphicsReady(signal) {
    return new Promise((resolve) => {
        const finish = (available) => {
            signal.removeEventListener('abort', abort);
            resolve(available);
        };
        const abort = () => { finish(false); };
        signal.addEventListener('abort', abort, { once: true });
        if (signal.aborted) {
            finish(false);
            return;
        }
        void (async () => {
            try {
                const info = await app.getGPUInfo('complete');
                if (signal.aborted)
                    return;
                const d3d11 = typeof info === 'object' && info !== null && 'auxAttributes' in info
                    && typeof info.auxAttributes === 'object' && info.auxAttributes !== null
                    && 'displayType' in info.auxAttributes && info.auxAttributes.displayType === 'ANGLE_D3D11';
                finish(d3d11 && app.isHardwareAccelerationEnabled() && app.getGPUFeatureStatus().gpu_compositing === 'enabled');
            }
            catch (_error) {
                // A failed GPU process cannot export Direct3D textures.
                finish(false);
            }
        })();
    });
}
function probeFrameForwardingAdapter(getAdapter, timeoutMs, signal) {
    return new Promise((resolve) => {
        let window;
        let settled = false;
        const finish = (adapter) => {
            if (settled)
                return;
            settled = true;
            clearTimeout(timer);
            signal.removeEventListener('abort', abort);
            if (window !== undefined && !window.isDestroyed())
                window.destroy();
            resolve(adapter);
        };
        const timer = setTimeout(() => { finish(null); }, timeoutMs);
        const abort = () => { finish(null); };
        signal.addEventListener('abort', abort, { once: true });
        try {
            window = new BrowserWindow({ show: false, width: 16, height: 16, webPreferences: {
                    sandbox: true, contextIsolation: true, nodeIntegration: false, offscreen: { useSharedTexture: true },
                } });
            if (settled) {
                if (!window.isDestroyed())
                    window.destroy();
                return;
            }
            window.on('closed', () => { finish(null); });
            window.webContents.on('render-process-gone', () => { finish(null); });
            window.webContents.on('paint', (event) => {
                const texture = event.texture;
                if (texture === undefined)
                    return;
                let adapter = null;
                try {
                    const handle = texture.textureInfo.handle.ntHandle;
                    if (handle !== undefined)
                        adapter = getAdapter(handle);
                }
                catch (_error) {
                    // Device reset or unsupported exported handles leave the adapter unknown.
                }
                finally {
                    texture.release();
                }
                finish(adapter);
            });
            void window.loadURL('data:text/html,<html><head><meta http-equiv="Content-Security-Policy" content="default-src %27none%27"></head><body>x</body></html>')
                .catch(() => { finish(null); });
        }
        catch (_error) {
            // Hardware acceleration can be unavailable before a probe window exists.
            finish(null);
        }
    });
}
