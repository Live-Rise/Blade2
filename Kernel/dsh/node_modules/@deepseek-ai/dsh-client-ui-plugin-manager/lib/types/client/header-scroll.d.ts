/** Scroll-linked header geometry keeps both responsive layouts without changing the list's flow height. */
/** Entry-owned measurement callbacks for the plugin page. */
export interface PluginHeaderLayoutFace {
    /** Create one element-ref lifecycle for each mounted page header. */
    createHeaderRef: () => (element: HTMLDivElement | null) => void;
}
/**
 * Measure the existing resting and compact CSS layouts in an inert, out-of-flow copy.
 * Their independent flex wrapping requires pixel measurements; interpolating only positioned
 * contents leaves the real header's resting footprint and the scroll range unchanged.
 * @returns A ref callback that disposes its element observers and scroll listener on detachment.
 */
export declare function createPluginHeaderRef(): (element: HTMLDivElement | null) => void;
//# sourceMappingURL=header-scroll.d.ts.map