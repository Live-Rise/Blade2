/** Sandboxed Electron texture receiver; application IPC and view authorization remain outside this module. */
import { sharedTexture } from 'electron';
/**
 * Install Electron's document-wide receiver before requesting production.
 * @returns Registration owner; each delivered lease independently retains its frame until release.
 */
export function createFrameForwardingTextureReceiver() {
    const listeners = new Map();
    sharedTexture.setSharedTextureReceiver(async ({ importedSharedTexture: imported }, metadata) => {
        let frame;
        let release;
        try {
            const listener = listeners.get(metadata.viewId);
            if (listener === undefined)
                return;
            frame = imported.getVideoFrame();
            const finished = Promise.withResolvers();
            let released = false;
            release = () => {
                if (released)
                    return;
                released = true;
                frame?.close();
                finished.resolve();
            };
            listener({ transport: 'shared-texture', metadata, frame, release });
            // The listener returning does not mean its consumer or GPU has finished reading.
            await finished.promise;
        }
        finally {
            release?.();
            imported.release();
        }
    });
    return {
        onFrame(id, listener) {
            if (listeners.has(id))
                throw new Error('Native view already has a frame consumer');
            listeners.set(id, listener);
            return () => {
                if (listeners.get(id) === listener)
                    listeners.delete(id);
            };
        },
        revoke(id) { listeners.delete(id); },
    };
}
