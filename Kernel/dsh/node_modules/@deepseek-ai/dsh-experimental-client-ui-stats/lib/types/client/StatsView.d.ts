/** Statistics view entry: scrolling table, paging state, and footer controls. */
import { type ReactElement } from 'react';
import { type StatsViewProps, type StatsViewInjected } from './StatsTable.tsx';
export type { StatsViewInjected, StatsViewProps };
/**
 * Render the Statistics table of the current conversation view target. The
 * table owns the viewport scroll: reaching the top asks the injected loader
 * for one older page while the already published turns stay in place.
 * @param props - composed conversation View props: the stats snapshot hook, the older-page loader, and the locale seat.
 * @returns the scrolling table, its empty state, and the expand/collapse footer.
 */
export declare function StatsView(props: StatsViewProps): ReactElement;
//# sourceMappingURL=StatsView.d.ts.map