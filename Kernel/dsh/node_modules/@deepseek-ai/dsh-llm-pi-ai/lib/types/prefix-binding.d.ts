/**
 * Request-prefix binding for replayed Anthropic thinking.
 *
 * Anthropic can bind each signed thinking block to the request prefix it was
 * generated under: system prompt, tools, and every earlier message. Compaction
 * and tool-result pruning rewrite that prefix, so a retained signed block
 * becomes invalid and endpoints that enforce the check reject the whole
 * request. Each Anthropic Messages response records a digest of the context it
 * was generated from; a later request replays its signed thinking only while
 * the reconstructed prefix still has the same digest. Replay state without a
 * digest predates this check and is treated as stale.
 *
 * The digest covers the pi-ai context objects this package builds, including
 * fields Anthropic never receives. Any change to their construction
 * invalidates every recorded digest once; a pi-ai upgrade that changes how
 * Anthropic messages are serialized without changing those objects requires a
 * new {@link DIGEST_SEED}.
 *
 * @module dsh-llm-pi-ai/prefix-binding
 */
import type { Api, AssistantMessage, Context as PiContext } from '@earendil-works/pi-ai';
/** Recorded request-prefix digests of reconstructed assistant messages, owned by one context conversion. */
export type PrefixDigests = Map<AssistantMessage, string>;
/** Context whose stale thinking is removed, with the digest of the complete request. */
export interface PrefixBoundContext {
    /** Context to send; equal to the input when nothing was removed. */
    context: PiContext;
    /** Digest of the sent context, recorded with the response; absent for other protocols. */
    digest?: string;
}
/**
 * Remove signed thinking from replayed Anthropic Messages assistant messages
 * whose request prefix changed since they were generated, or whose replay
 * state records no prefix digest. Manual thinking requires the final
 * assistant turn of a tool round to start with a thinking block, so with
 * `manualThinking` the assistant message whose tool round the request
 * continues keeps its thinking.
 * @param context - converted request context.
 * @param api - wire protocol of the requested model.
 * @param prefixDigests - recorded digests the context conversion collected.
 * @param manualThinking - whether the request enables budget-based rather than adaptive thinking.
 * @param onOmitted - called once with the number of messages whose thinking was removed, when there are any.
 * @returns the context to send and, for Anthropic Messages, its digest.
 */
export declare function bindThinkingToPrefix(context: PiContext, api: Api, prefixDigests: PrefixDigests, manualThinking: boolean, onOmitted: (count: number) => void): PrefixBoundContext;
//# sourceMappingURL=prefix-binding.d.ts.map