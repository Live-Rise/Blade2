/** Ordered presentation and capture of imported native images. */
import { createFrameForwardingFramePainter } from "./painters.js";
/** Serialized canvas presenter; accepts ownership of every submitted lease. */
export class FrameForwardingFrameDisplay {
    painter;
    queue = Promise.resolve();
    closed = false;
    displayed;
    generation = 0;
    revision = -1;
    waiting = new AbortController();
    disposal;
    constructor(painter) {
        this.painter = painter;
    }
    /**
     * Allocate a display canvas using the requested backend.
     * @param backend - Auto uses Canvas2D; explicit GPU backends support comparative measurements.
     * @param replaceCanvas - Mount a newly allocated canvas, replacing any previous one.
     * @param makeCanvas - Canvas allocator; each backend attempt needs a fresh element.
     * @returns Presenter whose dispose method must be called on unmount.
     */
    static async create(backend, replaceCanvas, makeCanvas = () => document.createElement('canvas')) {
        const painter = await createFrameForwardingFramePainter(backend, makeCanvas);
        try {
            replaceCanvas(painter.canvas);
        }
        catch (error) {
            painter.dispose();
            throw error;
        }
        return new FrameForwardingFrameDisplay(painter);
    }
    /** Active display backend. */
    get backend() { return this.painter.backend; }
    /**
     * Reject older viewport images while retaining the visible canvas until a replacement arrives.
     * @param version - Current viewport generation.
     */
    setGeneration(version) {
        if (version < this.generation)
            throw new Error('Native display generation cannot decrease.');
        if (version === this.generation)
            return;
        this.waiting.abort();
        this.waiting = new AbortController();
        this.generation = version;
        this.revision = -1;
    }
    waitUntilVisible(signal) {
        return new Promise((resolve) => {
            const finish = (visible) => {
                document.removeEventListener('visibilitychange', changed);
                signal.removeEventListener('abort', aborted);
                resolve(visible);
            };
            const changed = () => { if (!document.hidden)
                finish(true); };
            const aborted = () => { finish(false); };
            document.addEventListener('visibilitychange', changed);
            signal.addEventListener('abort', aborted, { once: true });
        });
    }
    /**
     * Paint in submission order and release the lease, including stale and failed images.
     * Hidden documents wait for visibility; the view owner must separately stop production through its viewport.
     * @param lease - Transferred ownership of a single frame.
     * @returns Whether the image became current; viewport changes or disposal cancel visibility waits.
     */
    present(lease) {
        const operation = this.queue.then(async () => {
            try {
                if (this.closed || lease.metadata.generation !== this.generation || lease.metadata.revision < this.revision)
                    return false;
                const signal = this.waiting.signal;
                if (document.hidden && !await this.waitUntilVisible(signal))
                    return false;
                if (signal.aborted)
                    return false;
                await this.painter.paint(lease.frame);
                if (this.closed || lease.metadata.generation !== this.generation)
                    return false;
                this.displayed = { ...lease.metadata };
                this.revision = lease.metadata.revision;
                return true;
            }
            finally {
                lease.release();
            }
        });
        this.queue = operation.then(() => { }, () => { });
        return operation;
    }
    /**
     * Capture after earlier submissions, with their exact displayed identity.
     * @param viewport - Viewport associated with the requested observation.
     * @returns PNG pixels, displayed metadata, and viewport; rejects stale/empty views.
     */
    capture(viewport) {
        const operation = this.queue.then(async () => {
            const metadata = this.displayed;
            if (this.closed || metadata === undefined || metadata.generation !== viewport.version || this.generation !== viewport.version)
                throw new Error('No native frame for the requested viewport.');
            const image = await new Promise((resolve, reject) => {
                this.painter.canvas.toBlob((blob) => {
                    if (blob === null)
                        reject(new Error('Native frame capture failed.'));
                    else
                        resolve(blob);
                }, 'image/png');
            });
            return { image, metadata: { ...metadata }, viewport: { ...viewport } };
        });
        this.queue = operation.then(() => { }, () => { });
        return operation;
    }
    /** Release every queued frame and dispose graphics resources.
     * @returns Completion after all owned frames and graphics resources are released.
     */
    async dispose() {
        if (this.disposal !== undefined)
            return this.disposal;
        this.closed = true;
        this.waiting.abort();
        this.disposal = this.queue.then(() => this.painter.dispose());
        return this.disposal;
    }
}
