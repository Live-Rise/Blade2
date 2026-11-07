/** Read immutable plan arguments from a Session history page and earlier pages. */
import type { Context } from '@deepseek-ai/cordis';
import type { ResourceProvider } from '@deepseek-ai/dsh-client-resources/client';
import { type SubmittedPlan } from './plan.ts';
declare module '@deepseek-ai/dsh-typert-protocol' {
    interface RemoteErrorDetailsMap {
        /** The resource URL does not identify a plan invocation. */
        'plan/invalid-address': Record<string, never>;
        /** The Session has no readable plan for this invocation. */
        'plan/not-found': Record<string, never>;
        /** The Session history read failed. */
        'plan/read-failed': Record<string, never>;
    }
}
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface ResourceProtocolMap {
        /** Immutable Markdown from a logged exit_plan_mode invocation. */
        plan: SubmittedPlan;
    }
}
/**
 * Bind plan reads to the generated Session Remote face.
 * A plan is a logged tool-call argument, so the provider reads one page at the current log
 * end and pages backwards from its cut. It neither activates nor subscribes.
 * Remote failures are reported as resource failure frames with their original codes.
 * @param remote - Existing Session history API.
 * @returns a provider whose reads stop after finding the exact invocation.
 */
export declare function planResourceProvider(remote: Pick<Context['remote']['session'], 'events' | 'page'>): ResourceProvider<'plan'>;
//# sourceMappingURL=plan-resource.d.ts.map