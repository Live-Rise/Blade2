import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { JsonRpcTransportPeer } from '@deepseek-ai/dsh-sdk-protocol';
/** Owns question forwarding and cancellation for one SDK transport. */
export declare class SdkUserQuestionBridge {
    private readonly ctx;
    private readonly transport;
    /** Enabled only after the host advertises an answerer at initialization. */
    enabled: boolean;
    private readonly questionShutdown;
    private readonly unregister;
    constructor(ctx: Context, transport: JsonRpcTransportPeer, ownsAgent: (agent: Agent) => boolean);
    /** Stop admission and cancel outstanding questions before SDK Agents drain. */
    close(): void;
    private ask;
}
//# sourceMappingURL=user-questions.d.ts.map