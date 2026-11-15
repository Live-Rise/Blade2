/** Cached code lettering clipped by the same frames that drive the dotted whale. */
import type { WhaleArtwork } from './artwork.ts';
interface CodeFill {
    resize(canvas: HTMLCanvasElement, width: number): void;
    paint(ctx: CanvasRenderingContext2D, pixels: Uint8ClampedArray, frame: number, left: number, top: number, width: number, level: number): void;
    dispose(): void;
}
/**
 * Allocate lettering and alpha canvases for one home attachment.
 * @param options - authored code phrases and font size.
 * @param atlas - bundled frame geometry; green stores opacity.
 * @returns a drawing cache, or undefined when Canvas 2D is unavailable.
 */
export declare function createCodeFill(options: WhaleArtwork, atlas: {
    width: number;
    height: number;
    columns: number;
    frames: number;
}): CodeFill | undefined;
export {};
//# sourceMappingURL=code-fill.d.ts.map