/** Home frame layer: a decorative canvas below the Conversation content, played by clicks on blank home space. */
import { type ReactNode } from 'react';
import type { HeroFrameOwnerProps } from '@deepseek-ai/dsh-client-ui-conversation/client';
/** Frame owner state and the wrapped content. */
export type PixelWhaleProps = HeroFrameOwnerProps & {
    children: ReactNode;
};
/**
 * Wrap the Conversation scroll body; while the home is shown, paint the whale canvas below the Conversation content.
 * @param props - the home state and the wrapped scroll body.
 * @returns a layout-neutral frame around the children.
 */
export declare function PixelWhale({ hero, children }: PixelWhaleProps): import("react").JSX.Element;
//# sourceMappingURL=PixelWhale.d.ts.map