/**
 * Fork orchestration for one viewer: ceiling decisions, child preparation, replay, and branch facts.
 *
 * The owner's own fork runs unchanged through the Gateway; this module decides whether it may run at
 * all, records the child the owner created under the logical request so a retry after that record
 * replays it, and records the child before anything can prompt it. Branch facts the reader was given
 * are added to list and projection answers here.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-fork
 */
import { type ForkLineage } from '../fork-lineage.ts';
import type { ShareRpcActor } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
import { type ViewerAnswer } from '../viewer-channel.ts';
import type { ForkPromptDecision } from './fork-registry.ts';
import type { ViewerReadDeps } from './viewer-read.ts';
import type { ViewerLifecycle } from './viewer-lifecycle.ts';
import { type ViewerDispatch } from './viewer-dispatch.ts';
/** Host services and orchestration one fork request reads. */
export interface ViewerForkDeps {
    /** Host services the reader was built with. */
    readonly deps: ViewerReadDeps;
    /** Lifecycle refusal answers and the enforcement records a child holds. */
    readonly lifecycle: ViewerLifecycle;
    /** The request dispatch the owner's own fork is made through. */
    readonly dispatch: () => ViewerDispatch;
}
/** Fork orchestration one viewer reader exposes. */
export interface ViewerFork {
    /**
     * Answer one fork request from a viewer.
     * @param shareId - Share the request arrived through.
     * @param path - Owner endpoint path to dispatch the fork to.
     * @param body - Opaque fork body the viewer sent.
     * @param actor - Viewer identity the Relay asserted.
     * @param signal - Cancellation of the forwarded request.
     * @returns The owner's own fork answer, or a named refusal.
     */
    answerFork(shareId: string, path: string, body: unknown, actor: ShareRpcActor, signal?: AbortSignal): Promise<ViewerAnswer>;
    /**
     * Activate one cold child through the composition, then decide again.
     * @param childSessionId - Child to prepare.
     * @param viewer - Identity the Relay asserted.
     * @param previous - Decision the registry gave before preparation.
     * @returns The decision after the activation.
     */
    prepareColdChild(childSessionId: string, viewer: ShareRpcActor, previous: ForkPromptDecision): Promise<ForkPromptDecision>;
    /**
     * Add the branch facts this reader was given to its own forks in one `session/list` answer.
     * @param answer - Owner's own `session/list` answer.
     * @param shareId - Share the request arrived through.
     * @param actor - Viewer identity the Relay asserted.
     * @returns The answer, with this reader's forks annotated for its sidebar.
     */
    annotateSessionList(answer: ViewerAnswer, shareId: string, actor: ShareRpcActor): ViewerAnswer;
    /**
     * Branch facts one reader was given for one Session, or undefined when it is not that reader's branch.
     * @param shareId - Share the read arrived through.
     * @param sessionId - Session the read names.
     * @param actor - Viewer identity the Relay asserted.
     * @returns The branch facts to publish, or undefined.
     */
    lineageFor(shareId: string, sessionId: string, actor: ShareRpcActor): ForkLineage | undefined;
    /**
     * One owner answer with the branch facts added to its page's projection block.
     * @param answer - The owner gateway's own answer.
     * @param lineage - Branch facts for the Session that answer describes.
     * @returns The answer, with the facts added to its projections.
     */
    withLineageAnswer(answer: ViewerAnswer, lineage: ForkLineage): ViewerAnswer;
    /**
     * Add the branch facts this reader was given wherever a relayed stream carries a projection block.
     * @param values - Frames the owner gateway streams for one endpoint.
     * @param endpoint - Stream endpoint being relayed.
     * @param requested - Session the stream was opened for, when its open item named one.
     * @returns The same frames, with the facts added to their projection blocks.
     */
    withLineageFrames(values: AsyncIterable<unknown>, endpoint: string, requested: string | undefined, lineageOf: (sessionId: string) => ForkLineage | undefined): AsyncGenerator;
}
/**
 * Build the fork orchestration for one viewer reader.
 * @param input - Host services, lifecycle state, and the request dispatch.
 * @returns Fork orchestration and branch annotations.
 */
export declare function createViewerFork(input: ViewerForkDeps): ViewerFork;
//# sourceMappingURL=viewer-fork.d.ts.map