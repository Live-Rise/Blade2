import type { Context } from '@deepseek-ai/cordis';
import { Config } from './config.ts';
import type { PiAiRequestHeaderContext } from './types.ts';
export { PiAiAdapter } from './adapter.ts';
export type { PiAiAdapterOptions } from './adapter.ts';
export type { PiAiRequestHeaderContext } from './types.ts';
export { Config } from './config.ts';
export type { Options, PiAiCompatProfile, PiAiModality, PiAiModelOverride, PiAiModelProfile, PiAiProviderProfile, PiAiReasoningEfforts, PiAiThinkingFormat, ResolvedPiAiProviderProfile, } from './config.ts';
export { recordKeyFor } from './auth.ts';
export { supportedProtocols } from './provider.ts';
export declare const name = "llm-pi-ai";
export declare const inject: string[];
declare module '@deepseek-ai/cordis' {
    interface Events {
        /**
         * Prepare headers for one pi-ai inference call, excluding model discovery.
         * Listeners run synchronously and must call next(); they may edit only the
         * request-owned headers and must not retain them. Throws fail the call with
         * REQUEST_HEADERS before inference I/O. Harness attribution wins afterward.
         * Non-TypeError messages are user-visible and must exclude secrets. TypeError
         * details and non-Error values are replaced with safe guidance.
         * @mode waterfall
         * @param request - immutable routing metadata from the exact prepared call.
         * @param headers - fresh profile headers, shared only within this dispatch.
         * @param next - continue header preparation through the remaining listeners.
         * @returns headers passed to pi-ai after Harness attribution is applied.
         */
        'llm-pi-ai/request-headers'(request: PiAiRequestHeaderContext, headers: Headers, next: () => Headers): Headers;
    }
}
/** Register one generic pi-ai adapter for all configured provider routes. */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map