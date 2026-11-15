/**
 * The `subagent` family of the Workspace registry's archive admission: which
 * subagent descendants of a Session are still inside a turn, and how they
 * stop when the Session is archived with its work. Archived ancestors also
 * block later model steps, including when their Sessions have been released.
 *
 * @module @deepseek-ai/dsh-subagent
 */
import type { Context } from '@deepseek-ai/cordis';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { Session, SessionId } from '@deepseek-ai/dsh-session';
import type { SubagentIdentityProjection } from './projection-types.ts';
/**
 * Answer `workspace/session-activity` with the running subagent descendants of
 * the asked Session, and `workspace/session-stop` by cancelling each of them
 * as their parent would. Reject subsequent steps beneath archived ancestors.
 * All listeners live as long as `ctx`'s fiber.
 * @param ctx - context carrying the Agent registry.
 * @param residentAncestors - verified creation ancestors for an exact managed Agent, absent before publication or for unmanaged Agents.
 * @param identityOf - ready service-owned descriptor identity reader; absent or invalid descriptors yield no identity.
 */
export declare function installSubagentArchiveAdmission(ctx: Context, residentAncestors: (agent: Agent) => readonly SessionId[] | undefined, identityOf: (session: Session) => SubagentIdentityProjection | undefined): void;
//# sourceMappingURL=archive-admission.d.ts.map