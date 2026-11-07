/** Refresh glyph that turns while a refresh runs. */
import { type ReactNode } from 'react';
/** Props of the refresh glyph. */
export interface RefreshGlyphProps {
    /** Whether the refresh runs. */
    readonly busy: boolean;
    /** Glyph size in pixels. */
    readonly size: number;
}
/**
 * Render the refresh glyph. It turns while `busy` and, once `busy` ends,
 * completes the turn under way, so a quick refresh still shows a whole turn
 * and the glyph stops upright. Under reduced motion it dims and returns instead of turning.
 * @param props - whether the refresh runs, and the glyph size.
 * @returns the glyph.
 */
export declare function RefreshGlyph({ busy, size }: RefreshGlyphProps): ReactNode;
//# sourceMappingURL=RefreshGlyph.d.ts.map