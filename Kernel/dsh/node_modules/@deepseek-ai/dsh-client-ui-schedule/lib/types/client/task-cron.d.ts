import type { Translate } from '@deepseek-ai/dsh-client-ui-slots';
/** ISO weekday dictionary key naming one cron day-of-week value. */
type CronWeekdayKey = `frequency.weekday.${1 | 2 | 3 | 4 | 5 | 6 | 7}`;
/** Dictionary keys the cron description renders. */
export type CronDescriptionKey = CronWeekdayKey | 'cron.list.join' | 'cron.part.join' | 'cron.weekday.name' | 'cron.weekday.range' | 'cron.months' | 'cron.day.every' | 'cron.day.weekdays' | 'cron.day.monthDays' | 'cron.day.both' | 'cron.day.bothStarred' | 'cron.hours.range' | 'cron.hours.list' | 'cron.time.everyMinute' | 'cron.time.everyMinutes' | 'cron.time.joinedEveryMinute' | 'cron.time.joinedEveryMinutes' | 'cron.time.everyHour' | 'cron.time.joinedEveryHour' | 'cron.time.everyNHours' | 'cron.time.joinedEveryNHours' | 'cron.time.hourlyAt' | 'cron.time.joinedHourlyAt' | 'cron.time.hoursEveryMinute' | 'cron.time.hoursEveryMinutes' | 'cron.time.at' | 'cron.time.hoursAt';
/** Namespace-independent translator for the cron description. */
export type CronDescriptionTranslator = Translate<CronDescriptionKey>;
/** One parsed cron field: its matched values, its Vixie star flag, and its coverage. */
export interface CronField {
    /** Unique ascending matched values; Sunday 7 is folded onto 0. */
    readonly values: readonly number[];
    /**
     * Whether the field text starts with `*`, which is Vixie's `DOM_STAR`/`DOW_STAR`
     * test the Host applies: a stepped star such as `*​/2` counts, while a full range
     * written out (`1-31`) does not.
     */
    readonly starred: boolean;
    /** Whether the matched set is every value the field can take. */
    readonly full: boolean;
    /** Whether the field is a star field that matches every value. */
    readonly unrestricted: boolean;
}
/** One parsed five-field cron expression in evaluation order. */
export interface ParsedCron {
    /** Unique ascending matched minutes. */
    readonly minutes: CronField;
    /** Unique ascending matched hours. */
    readonly hours: CronField;
    /** Unique ascending matched days of the month. */
    readonly daysOfMonth: CronField;
    /** Unique ascending matched months. */
    readonly months: CronField;
    /** Unique ascending matched cron weekdays, Sunday 0 through Saturday 6. */
    readonly daysOfWeek: CronField;
}
/**
 * Parse one five-field cron expression in the dialect the Host accepts.
 * @param expression - candidate `minute hour day-of-month month day-of-week` text.
 * @returns parsed fields, or undefined when the Host would reject the expression.
 */
export declare function parseCronExpression(expression: string): ParsedCron | undefined;
/**
 * Describe one parsed cron expression as one localized sentence.
 * @param parsed - parsed cron expression.
 * @param t - cron translations.
 * @param locale - active UI locale naming restricted months.
 * @returns localized sentence, for example `Mon–Fri at 09:00` or `Every 15 minutes`.
 */
export declare function cronPreview(parsed: ParsedCron, t: CronDescriptionTranslator, locale: string): string;
export {};
//# sourceMappingURL=task-cron.d.ts.map