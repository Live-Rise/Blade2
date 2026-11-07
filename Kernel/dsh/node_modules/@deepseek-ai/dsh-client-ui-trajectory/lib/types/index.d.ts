import { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import type { SessionToolResultRequest, SessionToolResultValue } from './types.ts';
export type { SessionToolResultRequest, SessionToolResultValue } from './types.ts';
/** Count limit for each live Session's transient tool results. */
export interface Config {
    /** Maximum retained results per Session, including values awaiting a result event. */
    maxResultsPerSession: number;
}
/** Read-only Remote inspection of tool values produced in this Session lifetime. */
export default class SessionToolResults extends TypertRemoteService {
    static inject: string[];
    static Config: z<Partial<Config>, Config>;
    private readonly results;
    private readonly lifetime;
    /**
     * @param ctx - Owner of the live Session registry and tool result notifications.
     * @param config - Validated per-Session retention limit.
     */
    constructor(ctx: Context, config: Config);
    private forSession;
    private forRequest;
    /**
     * Observe one committed result without loading a Session or reading its history.
     * Values disappear on count eviction or when the exact Session instance leaves
     * the registry. A cold read stays unavailable; opening a new stream observes
     * the current instance. Inspection does not extend a result's retention.
     * @param request - Authorized Session identity and durable result event sequence.
     * @param signal - Remote consumer lifetime.
     * @returns Current availability and changes until cancellation or service disposal.
     */
    watch(request: SessionToolResultRequest, signal: AbortSignal): AsyncIterable<SessionToolResultValue>;
}
//# sourceMappingURL=index.d.ts.map