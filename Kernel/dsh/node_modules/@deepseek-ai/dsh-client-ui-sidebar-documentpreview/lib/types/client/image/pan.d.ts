/** Mouse panning for overflowing image previews; contained images keep native file dragging. */
/**
 * Bind image panning to its existing scrollport without changing zoom or replacing the image.
 * @param image - mounted preview image inside a document zoom scrollport.
 * @returns cleanup releasing observation, pointer capture, and window listeners.
 */
export declare function bindImagePan(image: HTMLImageElement): () => void;
//# sourceMappingURL=pan.d.ts.map