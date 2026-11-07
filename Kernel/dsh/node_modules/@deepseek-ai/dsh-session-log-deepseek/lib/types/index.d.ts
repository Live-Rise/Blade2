/**
 * Incremental session-log contribution for official DeepSeek LLM API requests.
 * Accepted sequence watermarks live in the canonical log, so restart recovery
 * can conservatively resend uncertain tails without maintaining another store.
 * @module @deepseek-ai/dsh-session-log-deepseek
 */
import { Service, type Context, type Volatile } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Session, SessionSeqCursor } from '@deepseek-ai/dsh-session';
import type { SessionLogUploadExclusion } from './types.ts';
export type * from './types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        sessionLogDeepSeek: SessionLogDeepSeekService;
    }
}
/** Cordis plugin name. */
export declare const name = "session-log-deepseek";
/** Services required to resolve sessions and contribute the provider request field. */
export declare const inject: string[];
/** Session-log request contribution configuration. */
export interface Config {
    /** Contribute `dsh_session_log` to official DeepSeek requests. Defaults to `true`. */
    enabled: Volatile<boolean>;
    /**
     * Largest serialized `dsh_session_log` field, in UTF-8 bytes, that one request carries.
     * A request uploads the longest pending event prefix that fits; later requests continue
     * after its acceptance. Defaults to 8 MiB.
     */
    maxBytes: number;
}
/** Validated Session-log request contribution configuration. */
export declare const Config: z<Schemastery.ObjectS<NoInfer<{
    enabled: z<boolean, boolean, "volatile-defined">;
    maxBytes: z<number, number, "defined">;
}>>, Schemastery.ObjectT<NoInfer<{
    enabled: z<boolean, boolean, "volatile-defined">;
    maxBytes: z<number, number, "defined">;
}>>, "plain">;
/**
 * Highest confirmed sequence for this exact Session format generation.
 * @param session - canonical log whose matching acceptance events are folded.
 * @returns greatest accepted sequence, or `-1` before any accepted request.
 */
export declare function acceptedThrough(session: Session): SessionSeqCursor;
/** Incremental Session-log contributor with caller-owned upload exclusions. */
export declare class SessionLogDeepSeekService extends Service {
    static inject: string[];
    private readonly exclusions;
    constructor(ctx: Context, config: Config);
    /**
     * Exclude matching requests before reading or preparing their log contribution.
     * Any matching rule suppresses both the field and its acceptance callback. With
     * no matching rules, normal upload eligibility applies. A throwing rule fails
     * this contribution; the request-extension registry omits it and reports the error.
     * @param exclude - synchronous, side-effect-free predicate; `true` skips this upload.
     * @returns effect disposer that removes this registration when called or when the caller unloads.
     */
    registerExclusion(exclude: SessionLogUploadExclusion): () => Promise<void>;
}
/**
 * Register the incremental request contribution; enablement is read for each request.
 * @param ctx - plugin context carrying Sessions and the DeepSeek request-extension registry.
 * @param config - validated configuration.
 * @returns fulfillment after the upload service is mounted.
 */
export declare function apply(ctx: Context, config: Config): Promise<void>;
//# sourceMappingURL=index.d.ts.map