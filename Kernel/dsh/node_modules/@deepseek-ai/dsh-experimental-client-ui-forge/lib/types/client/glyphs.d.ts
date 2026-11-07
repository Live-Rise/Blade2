/** Lifecycle glyphs of change requests and outcome glyphs of their checks. */
import type { ReactNode } from 'react';
import type { CheckState } from '@deepseek-ai/dsh-experimental-forge/types';
import type { ChangeRequestTone } from './status.ts';
/**
 * Draw a change request in its lifecycle shape: open, open with a cross for
 * one that needs its author, draft, merged, closed, or the open shape for an
 * unknown state.
 * @param props.tone - lifecycle state that picks the shape and, when tinted, the color.
 * @param props.tinted - whether the state picks the color; otherwise the glyph uses the current text color.
 * @param props.size - width and height in pixels, 15 by default.
 * @param props.label - accessible name; omit it for a decorative glyph.
 * @returns the SVG glyph.
 */
export declare function ChangeRequestGlyph({ tone, tinted, size, label }: {
    readonly tone: ChangeRequestTone;
    readonly tinted?: boolean | undefined;
    readonly size?: number | undefined;
    readonly label?: string | undefined;
}): ReactNode;
/**
 * Draw the decorative outcome of one check beside its text: a filled circle
 * for success or failure, a half-filled ring while it runs, and an empty ring
 * for neutral, skipped, and cancelled checks.
 * @param props.state - check outcome.
 * @param props.size - width and height in pixels, 16 by default.
 * @returns the glyph.
 */
export declare function CheckGlyph({ state, size }: {
    readonly state: CheckState;
    readonly size?: number | undefined;
}): ReactNode;
//# sourceMappingURL=glyphs.d.ts.map