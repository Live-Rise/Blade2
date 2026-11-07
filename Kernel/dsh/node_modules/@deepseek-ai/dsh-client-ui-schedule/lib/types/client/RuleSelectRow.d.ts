import type { ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { TaskMenuEntry } from './TaskMenu.tsx';
/** A labelled menu row using the current task detail's trigger, portal, and keyboard behavior. */
export declare function RuleSelectRow({ label, value, current, items, disabled, onChange, onDismiss, header, listClassName }: {
    readonly label: string;
    readonly value: string;
    readonly current: string;
    readonly items: readonly TaskMenuEntry[];
    readonly disabled: boolean;
    readonly onChange: (value: string) => void;
    readonly onDismiss?: () => void;
    readonly header?: ReactNode;
    readonly listClassName?: string | undefined;
}): ReactNode;
/** The current task detail's searchable, localized zone menu with recent choices. */
export declare function RuleZoneRow({ value, disabled, onChange, t }: PropsLocale<'schedule.manager'> & {
    readonly value: string;
    readonly disabled: boolean;
    readonly onChange: (value: string) => void;
}): ReactNode;
//# sourceMappingURL=RuleSelectRow.d.ts.map