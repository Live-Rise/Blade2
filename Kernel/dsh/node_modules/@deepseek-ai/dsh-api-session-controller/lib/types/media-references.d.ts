/**
 * Authenticated GET/HEAD /api/file reads bounded file responses through
 * the composed filesystem provider. Paths and MIME types do not restrict access;
 * the connection service authenticates requests before this handler. A request
 * without the current `v` version is redirected, uncached, to the versioned URL,
 * whose response is cacheable indefinitely.
 * @module @deepseek-ai/dsh-api-session-controller/media-references
 */
import type { Context } from '@deepseek-ai/cordis';
/**
 * File-display contribution. The connection service supplies authentication;
 * `ctx.fs` supplies the execution world's paths, reads, and access policy.
 */
export declare const SessionMediaReferences: {
    inject: string[];
    apply(ctx: Context): void;
};
//# sourceMappingURL=media-references.d.ts.map