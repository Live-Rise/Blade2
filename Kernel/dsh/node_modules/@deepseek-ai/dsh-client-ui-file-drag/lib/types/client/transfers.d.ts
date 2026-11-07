/** File transport selected once per plugin activation; sources only receive prepared operations. */
import { type DesktopFileDrag } from '@deepseek-ai/dsh-util-file-drag';
import type { FileDragSource, PreparedFileDrag } from '@deepseek-ai/dsh-client-ui-primitives';
/** Application-local browser transfer owner, including detached source-node drag end. */
export declare class FileTransfers {
    private readonly desktop;
    private readonly reportFailure;
    private active;
    private readonly prepared;
    private disposed;
    private readonly lifetime;
    /** @param desktop - verified native bridge, absent in Web. @param reportFailure - localized plugin feedback. */
    constructor(desktop: DesktopFileDrag | undefined, reportFailure: () => void);
    /**
     * Resolve this instance's active transfer.
     * @param transfer - browser drop data.
     * @returns retained bytes only for this instance's current token.
     */
    read(transfer: DataTransfer): File | undefined;
    private assertOpen;
    private prepareNative;
    /**
     * Prepare bytes or borrow an authorized native file.
     * @param source - content plus export naming.
     * @param signal - caller cancellation.
     * @returns synchronous drag startup and preparation release.
     */
    prepare(source: FileDragSource, signal: AbortSignal): Promise<PreparedFileDrag>;
    private readSource;
    private native;
    private browser;
    /** End active transfers and retire every preparation, including sources detached during dragging. */
    dispose(): void;
}
//# sourceMappingURL=transfers.d.ts.map