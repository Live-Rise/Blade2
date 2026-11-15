/** Alternating dot and code breaches sharing one bundled animation atlas. */
import type { WhaleArtwork } from './artwork.ts';
/** Control of one attached whale decoration. */
export interface PixelWhaleHandle {
    /** Request one playback; ignored while playing, failed, hidden, or under reduced motion. */
    play: () => void;
    /** Release frame work, atlas pixels, and observers; late atlas loads are ignored. */
    dispose: () => void;
}
/**
 * Bind one noninteractive, initially transparent home decoration.
 * Missing Canvas support returns a no-op handle, so home stays usable without decoration.
 * An atlas failure stops decoration and logs a reload action.
 * @param canvas - presentation-owned output, with its neutral color supplied by CSS.
 * @param options - fixed opacity, lettering, frame rate, and duration artwork.
 * @param atlasUrl - URL of the packaged frame atlas, loaded once on the first eligible playback.
 * @returns the playback trigger and its teardown.
 */
export declare function attachPixelWhale(canvas: HTMLCanvasElement, options: WhaleArtwork, atlasUrl: string): PixelWhaleHandle;
//# sourceMappingURL=renderer.d.ts.map