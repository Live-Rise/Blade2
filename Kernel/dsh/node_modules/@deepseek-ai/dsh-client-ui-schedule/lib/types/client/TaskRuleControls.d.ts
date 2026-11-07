/** Shared rule controls taken from the existing task detail. */
import type { ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
type Weekday = 1 | 2 | 3 | 4 | 5 | 6 | 7;
/**
 * Render the weekly rule's Weekday row: one pill per weekday, named by the row's
 * label and toggled in the rule the caller is editing.
 * @param props.weekdays - weekdays the rule currently selects, as ISO weekday numbers (Monday is 1).
 * @param props.disabled - whether the rule's controls are read-only.
 * @param props.t - frequency translator owning the weekday labels.
 * @param props.rowId - id builder for the row's label, which names the pill group.
 * @param props.onToggle - toggle one weekday in the rule being edited.
 * @returns the labelled Weekday row.
 */
export declare function RuleWeekdayRow({ weekdays, disabled, t, rowId, onToggle }: {
    weekdays: readonly number[];
    disabled: boolean;
    t: PropsLocale<'schedule.manager'>['t'];
    rowId: (name: string) => string;
    onToggle: (weekday: Weekday) => void;
}): ReactNode;
/** Inputs of one whole-number stepper of the task rule form. */
interface RuleStepperProps {
    /** Id the row's label points at. */
    readonly id: string;
    /** Smallest value the field accepts. */
    readonly min: number;
    /** Largest value the field accepts. */
    readonly max: number;
    /** Staged value. */
    readonly value: number;
    /** Whether every edit is blocked. */
    readonly disabled: boolean;
    /** Accessible name of the up arrow. */
    readonly increaseLabel: string;
    /** Accessible name of the down arrow. */
    readonly decreaseLabel: string;
    /** Stage one clamped whole value. */
    readonly onChange: (value: number) => void;
}
/**
 * One whole-number stepper of the task rule form, such as the repeat interval or
 * the day of the month. Values clamp to the field's bounds, and an emptied or
 * fractional input stages nothing, so the staged rule stays valid on every edit.
 * @param props - bounds, staged value, blocking state, arrow labels, and callback.
 * @returns the stepper pill.
 */
export declare function RuleStepper({ id, min, max, value, disabled, increaseLabel, decreaseLabel, onChange }: RuleStepperProps): ReactNode;
export {};
//# sourceMappingURL=TaskRuleControls.d.ts.map