/** Electron Main ownership of a native texture through renderer and GPU release. */
import { sharedTexture } from 'electron';
/**
 * Transfer an accepted native frame to one renderer. The caller validates its recipient and metadata.
 * @param frame - Authorized renderer frame.
 * @param event - Native receiver delivery whose credit is transferred to this operation.
 * @param metadata - Application metadata forwarded unchanged to the renderer.
 * @param release - Return this delivery's credit after every Electron reference is released.
 * @param onFailure - Synchronously stop new producer I/O on failure, before references or credit can return.
 * @returns Completion of transport submission; GPU ownership may outlive this promise.
 */
export async function sendFrameForwardingFrame(frame, event, metadata, release, onFailure) {
    let imported;
    try {
        imported = sharedTexture.importSharedTexture({
            textureInfo: {
                handle: process.platform === 'darwin' ? { ioSurface: event.handle } : { ntHandle: event.handle },
                pixelFormat: 'bgra',
                codedSize: { width: event.width, height: event.height },
                colorSpace: { primaries: 'bt709', transfer: 'srgb', matrix: 'rgb', range: 'full' },
            },
            allReferencesReleased: release,
        });
        await sharedTexture.sendSharedTexture({ frame, importedSharedTexture: imported }, metadata);
    }
    catch (error) {
        onFailure(error);
        throw error;
    }
    finally {
        if (imported === undefined)
            release();
        else
            imported.release();
    }
}
