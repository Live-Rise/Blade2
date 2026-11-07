/** Chip title of a changes tab: the changes icon and the view's name in the current language. */
import type { ReactNode } from 'react';
import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
/** Props of the changes tab title. */
export type ChangesTabTitleProps = Pick<PropsRuntime<'sidebar.right.pane.tab.title'>, 'useTabInfo'> & PropsLocale<typeof NS>;
/**
 * Render the chip title of a changes tab; an address this package did not mint keeps the captured title.
 * @param props - tab information and forge panel copy.
 * @returns the icon followed by the title.
 */
export declare function ChangesTabTitle({ useTabInfo, t }: ChangesTabTitleProps): ReactNode;
//# sourceMappingURL=ChangesTabTitle.d.ts.map