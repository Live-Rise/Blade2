import type { ContextMenuBindings } from '@deepseek-ai/dsh-client-ui-context-menu/client';
import type { ImageAttachmentRef } from '@deepseek-ai/dsh-attachment';
import { type FileDragActions } from '@deepseek-ai/dsh-client-ui-primitives';
import type { MessageImageLabels } from '@deepseek-ai/dsh-client-ui-conversation/client';
export type { MessageImageLabels } from '@deepseek-ai/dsh-client-ui-conversation/client';
/** Loads a Gateway-authorized durable image URL and may expose a cached URL synchronously. */
export type ImageLoader = ((attachment: ImageAttachmentRef) => Promise<string>) & {
    peek?: (attachment: ImageAttachmentRef) => string | undefined;
};
/** One gallery entry: a durable admitted reference, or a submission echo's local preview. */
export type MessageImageSpec = {
    readonly attachment: ImageAttachmentRef;
    /** Presentation-only name for the thumbnail and lightbox; loading uses the original reference. */
    readonly label?: string;
} | {
    readonly preview: {
        readonly url: string;
        readonly name?: string;
        readonly width?: number;
        readonly height?: number;
    };
};
/**
 * Compact history renderer with retryable loading and click-to-open original
 * preview. A lone image renders at its `singleFit` size; an image among
 * several renders as a fixed 64px square tile. The preview arm displays its
 * local URL directly — no loader round-trip, no failure/retry surface.
 * List thumbnails use status icons with localized tooltips and accessible names.
 *
 * @param props.image - the durable reference to load, or the local preview to display.
 * @param props.load - Gateway-authorized URL loader for the durable arm.
 * @param props.variant - lone image, cropped gallery tile, or uncropped list thumbnail.
 * @param props.labels - resolved strings (tooltip, loading, retry, lightbox).
 * @returns the bounded thumbnail button, or the retry control on failure.
 */
export declare function MessageImage({ image, load, variant, labels, fileDrag, getTriggerProps }: {
    image: MessageImageSpec;
    load: ImageLoader;
    variant: 'single' | 'tile' | 'thumbnail';
    fileDrag: FileDragActions;
    labels: MessageImageLabels;
    getTriggerProps: ContextMenuBindings['getTriggerProps'];
}): import("react").JSX.Element;
/** Wrapping image group shared by user and assistant history: a lone image
 * renders large unless its owner requests compact tiles or contained list thumbnails. */
export declare function ImageGallery({ images, load, align, compact, thumbnail, labels, fileDrag, getTriggerProps }: {
    images: readonly MessageImageSpec[];
    load: ImageLoader;
    align: 'start' | 'end';
    compact?: boolean;
    thumbnail?: boolean;
    fileDrag: FileDragActions;
    labels: MessageImageLabels;
    getTriggerProps: ContextMenuBindings['getTriggerProps'];
}): import("react").JSX.Element | null;
//# sourceMappingURL=MessageImage.d.ts.map