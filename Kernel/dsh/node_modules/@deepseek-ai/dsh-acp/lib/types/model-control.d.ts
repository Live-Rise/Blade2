/** Standard ACP session configuration over one Agent's model selection. */
import type { SessionConfigOption } from '@agentclientprotocol/sdk';
import { type Model } from '@deepseek-ai/dsh-agent';
import { type LlmRuntime } from '@deepseek-ai/dsh-llm';
/** Caller-correctable session configuration failure. */
export declare class AcpModelConfigError extends Error {
    constructor(message: string);
}
/**
 * Project and mutate one Agent's provider/model/reasoning selection through ACP
 * config options. The Session's recorded model selection is the only state; a
 * change applies from the Agent's next model step.
 */
export declare class AcpModelControl {
    private readonly llm;
    private readonly model;
    private tail;
    private hasResolvedState;
    constructor(llm: LlmRuntime, model: Model);
    /**
     * Return the complete standard config-option state after prior mutations settle.
     * @param signal - optional catalog and exact-model cancellation.
     * @returns all current standard configuration options.
     */
    options(signal?: AbortSignal): Promise<SessionConfigOption[]>;
    /**
     * Set one advertised option and return the complete resulting option state.
     * @param configId - standard option id.
     * @param value - opaque selected value returned by a previous option state.
     * @param signal - optional catalog and exact-model cancellation.
     * @returns all standard options after the serialized mutation.
     */
    set(configId: string, value: unknown, signal?: AbortSignal): Promise<SessionConfigOption[]>;
    /** Keep concurrent client mutations in receive order without wedging after rejection. */
    private serialize;
    /** Build detached model choices and the dependent reasoning option. */
    private state;
    /** Validate an exact route and retain only Agent-owned selection fields. */
    private resolveSelection;
}
//# sourceMappingURL=model-control.d.ts.map