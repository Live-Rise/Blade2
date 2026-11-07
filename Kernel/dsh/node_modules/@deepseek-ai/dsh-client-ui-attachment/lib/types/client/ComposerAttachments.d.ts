import type { ComposerAttachmentsProps } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { InjectFace } from '@deepseek-ai/dsh-client-ui-slots';
/** File transfer lookup injected by this attachment implementation. */
export interface ComposerAttachmentsInjected {
    readonly readFileDrag: (transfer: DataTransfer) => File | undefined;
}
/** The owner share and locale seat the attachment rail reads from its slot props. */
type ComposerAttachmentsInput = Pick<ComposerAttachmentsProps, 'attachments' | 'canAcceptDrop' | 'onAddFiles' | 'onRemoveAttachment' | 'uploads' | 'onRetryFile' | 'uploadFailureText' | 'dropLimits' | 't'>;
/** Draft image previews, pending-file cards, drop target, and original-image preview. */
export declare function ComposerAttachments({ attachments, canAcceptDrop, onAddFiles, onRemoveAttachment, uploads, onRetryFile, uploadFailureText, dropLimits, t, readFileDrag, }: ComposerAttachmentsInput & InjectFace<ComposerAttachmentsInjected>): import("react").JSX.Element;
export {};
//# sourceMappingURL=ComposerAttachments.d.ts.map