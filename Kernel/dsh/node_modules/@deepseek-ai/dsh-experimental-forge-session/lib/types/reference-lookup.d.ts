/** Resolve unqualified CLI references in the shell execution world and persist only confirmed URLs. */
import type { Context } from '@deepseek-ai/cordis';
import type { Session } from '@deepseek-ai/dsh-session';
/**
 * Observe completed foreground Bash calls while tools and shell services exist.
 * Queries use the same shell service, cwd, managed environment, and standing
 * sandbox policy. Failed lookups leave bindings unchanged and log a warning.
 * Cancellation leaves bindings unchanged; disposal aborts and awaits all queries.
 * Replay performs no lookup.
 * @param ctx - owning plugin context.
 * @param timeoutMs - shell deadline for one read-only lookup.
 * @param record - persists a confirmed URL without restoring dismissed bindings.
 */
export declare function observeReferences(ctx: Context, timeoutMs: number, record: (session: Session, url: string) => void): void;
//# sourceMappingURL=reference-lookup.d.ts.map