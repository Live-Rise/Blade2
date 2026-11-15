/** Fixed artwork choices for the official home whale. */
/** Visual parameters shared by the dot and code fills. */
export interface WhaleArtwork {
    /** Maximum fill opacity, multiplied by each frame's water and highlight attenuation. */
    activeOpacity: number;
    /** Word size in CSS pixels. */
    fontSize: number;
    /** Authored artwork content, drawn verbatim as complete terms or sentences. */
    phrases: readonly string[];
    /** Maximum canvas redraw frequency. */
    frameRate: number;
    /** One click's total lifetime in milliseconds. */
    rippleDurationMs: number;
}
/** The official artwork; these values are part of the design, not deployment settings. */
export declare const WHALE_ARTWORK: WhaleArtwork;
//# sourceMappingURL=artwork.d.ts.map