/** Host-owned opt-in setting for model-selectable subagent delegation. */
import type { Volatile } from '@deepseek-ai/cordis';
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { type AllowedModelRoute } from './model-selection.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** User preference sampled during Agent creation. */
        subagentModelSelection: SubagentModelSelectionConfig;
    }
}
/** Stored user preference; the shipped composition defaults it off. */
export interface SubagentModelSelectionSettings {
    /** Whether new top-level Sessions receive model selection. */
    enabled: boolean;
    /** Exact child LLM routes offered to new top-level Sessions. */
    allowedModels: AllowedModelRoute[];
}
/** Optional deployment base for the preference. */
export interface Config {
    /** Initial enabled state inherited when the user document does not override it. */
    enabled: Volatile<boolean>;
    /** Initial route list inherited when the user document does not override it. */
    allowedModels: Volatile<AllowedModelRoute[]>;
}
/** Host settings owner that records Session authorization before the first model request. */
export declare class SubagentModelSelectionConfig extends Service {
    private config;
    static inject: string[];
    static Config: z<Schemastery.ObjectS<NoInfer<{
        enabled: z<boolean, boolean, "volatile-defined">;
        allowedModels: z<NoInfer<AllowedModelRoute[]>, NoInfer<AllowedModelRoute[]>, "volatile-defined">;
    }>>, Schemastery.ObjectT<NoInfer<{
        enabled: z<boolean, boolean, "volatile-defined">;
        allowedModels: z<NoInfer<AllowedModelRoute[]>, NoInfer<AllowedModelRoute[]>, "volatile-defined">;
    }>>, "plain">;
    constructor(ctx: Context, config: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Read a detached selection preference for the next fresh top-level Session.
     * @returns the enabled state and exact allowed routes.
     */
    private current;
}
export declare const name = "subagent-model-selection-settings";
export default SubagentModelSelectionConfig;
//# sourceMappingURL=model-selection-settings.d.ts.map