/** Public polling configuration shared by the notification Host and Client halves. */
import Schema from '@deepseek-ai/schemastery';
/** Initial polling interval before the server supplies its next delay. */
export interface Config {
    /** Initial query delay in milliseconds, replaced by each successful server response. */
    notificationPollIntervalMs: number;
}
/** Validate notification polling configuration. */
export declare const Config: Schema<Schemastery.ObjectS<NoInfer<{
    notificationPollIntervalMs: Schema<number, number, "defined">;
}>>, Schemastery.ObjectT<NoInfer<{
    notificationPollIntervalMs: Schema<number, number, "defined">;
}>>, "plain">;
/** Bootstrap key for public notification configuration. */
export declare const NOTIFICATIONS_CONFIG_GLOBAL = "__DSH_ACCOUNT_NOTIFICATIONS_CONFIG__";
//# sourceMappingURL=config.d.ts.map