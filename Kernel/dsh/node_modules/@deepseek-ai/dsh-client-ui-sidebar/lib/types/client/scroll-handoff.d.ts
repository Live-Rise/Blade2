/**
 * Wheel hand-off between the sidebar viewport and the section scroller below its navigation.
 * Scrolling down moves the navigation away before the section scroller moves; scrolling up
 * returns the section scroller to its top before the navigation reappears. A gesture whose
 * first event must move the viewport, or starts outside the section scroller, is split in
 * JavaScript until it ends, including its momentum; both elements are then written
 * synchronously in each event, so one gesture continues across the boundary. A gesture the
 * section scroller can take alone keeps native scrolling.
 */
import type { RefCallback } from 'react';
import type { UseSectionScroller } from './contract/slots.ts';
/** Sidebar-owned state of one hand-off. */
export interface ScrollHandoff {
    /** Ref callback for the sidebar's own viewport; binds the wheel listener. */
    readonly bindViewportRef: RefCallback<HTMLElement>;
    /** Hook handed to the `sidebar.workspaces` occupant through the slot's hookContext. */
    readonly useSectionScroller: UseSectionScroller;
}
/**
 * Create the hand-off for one sidebar render tree.
 * @returns The viewport ref callback and the occupant hook; both keep their identity.
 */
export declare function createScrollHandoff(): ScrollHandoff;
//# sourceMappingURL=scroll-handoff.d.ts.map