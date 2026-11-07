import type { ComposerAttachmentsProps } from '@deepseek-ai/dsh-client-ui-conversation/client';
/**
 * Install one attachment view's file-drop listeners.
 * @param canAcceptDrop - whether this view accepts the dropped files.
 * @param onAddFiles - attachment intake callback.
 * @param dragDepth - the view's retained nested-drag counter.
 * @param setDragActive - publish whether a file drag is active.
 * @param readFileDrag - resolve an original image retained by this application during a browser drag.
 * @returns cleanup for exactly these listeners.
 */
export declare function installDocumentDropEvents(canAcceptDrop: ComposerAttachmentsProps['canAcceptDrop'], onAddFiles: ComposerAttachmentsProps['onAddFiles'], dragDepth: {
    current: number;
}, setDragActive: (active: boolean) => void, readFileDrag: (transfer: DataTransfer) => File | undefined): () => void;
//# sourceMappingURL=drop-events.d.ts.map