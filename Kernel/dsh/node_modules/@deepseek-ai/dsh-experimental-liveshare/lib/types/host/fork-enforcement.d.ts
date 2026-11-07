/**
 * The host's real enforcement wiring for one forked child.
 *
 * Three parts must land before a child counts as enforced: a producer on the child's own scoped
 * runtime that resolves each execution's paths through the host's filesystem and records the verdict
 * for that exact execution, the monotonic guard that refuses whatever the verdict did not confirm,
 * and the fork-scoped replacements that check a resolved target against the granted root and delegate
 * with that target's canonical path.
 *
 * The child's authorized root is the workspace its own session was granted, never the owner's
 * deployment default. Containment is checked on the resolved path before the deployment's tool or the
 * backend opens it again by path, so a symlink swapped in between can redirect the operation; the
 * write still passes the backend's own sandbox policy. Kernel-grade isolation of untrusted code remains
 * `ctx.shell`'s job.
 */
import type { Context } from '@deepseek-ai/cordis';
import type { FsEditOutcome, FsEditRequest, FsTarget, FsVersion, FsWriteOutcome, FsWriteIntent } from '@deepseek-ai/dsh-fs';
import type { ToolDefinition } from '@deepseek-ai/dsh-tools';
import type { Session } from '@deepseek-ai/dsh-session';
import type { SandboxExecutionPolicy } from '@deepseek-ai/dsh-sandbox';
import { type ForkToolExecution } from './fork-tool-guard.ts';
import type { ForkRegistry } from './fork-registry.ts';
/** Why a source Session carries no usable confined layer. */
export type SourceSessionAccessRefusal = 
/** The deployment exposes no tool definition to replace. */
'tool-unavailable'
/** The replacement could not be registered in the Session's scope. */
 | 'register-refused';
/** A confined tool whose registration a scope refused, with the error that scope raised. */
export interface ForkToolRegistrationFailure {
    /** Name of the tool whose confined replacement could not register. */
    readonly tool: string;
    /** Error the scope raised, preserved so the caller's own refusal can state it. */
    readonly error: unknown;
}
/** What installing the source-session access layer achieved. */
export interface SourceSessionAccessInstallation {
    /** Release the guard, and every replacement this installation registered. */
    dispose(): void;
    /**
     * Whether this installation confines the Session's view surface inside the granted workspace.
     *
     * The read replacement is the anchor: its absence means nothing usable was installed, a viewer-started
     * turn is refused every tool while this layer is mounted, and a caller must not report the share as
     * drivable or drive the prompt. Read-only tools the deployment does not ship stay unavailable without
     * clearing this flag.
     */
    readonly readConfined: boolean;
    /** Whether this installation can enforce a bounded write through the backend. */
    readonly writeConfined: boolean;
    /** Why the layer is unusable, set only when {@link readConfined} is false. */
    readonly refusal?: SourceSessionAccessRefusal;
    /**
     * The confined replacement a scope refused, when one registration failed.
     *
     * Fail-closed installation returns a narrower layer instead of throwing; this keeps the error the scope
     * raised so the host can report it rather than losing the reason.
     */
    readonly registrationFailure?: ForkToolRegistrationFailure;
}
/** The Session whose access this is, and everything the wrapper needs to judge one execution. */
export interface SourceSessionAccess {
    /** Session whose tool executions are constrained, for messages and audit. */
    readonly sessionId: string;
    /** That Session's own scope, which the replacements are registered into. */
    readonly context: Context;
    readonly scope: object;
    /** Workspace this Session's tool executions may read and write inside. */
    readonly grantedRoot: FsTarget;
    readonly cwd: string;
    /** Resolve this Session's complete current policy after write-intent listeners settle. */
    readonly resolvePolicy: () => SandboxExecutionPolicy;
    readonly fs: ForkEnforcementFs;
    readonly tools: ForkToolRegistry;
    readonly describe: (execution: ForkToolExecution) => ForkToolPaths;
    /**
     * Present when this installation registers the bounded write, absent when the Session is read-only.
     *
     * A Session shared only at the view tier installs no write replacement, so the guard refuses the name.
     * A Session some share may write installs one, and the field beside it answers whether writing is still
     * allowed: the tier of the Session's shares can narrow while the layer stays mounted.
     */
    readonly write?: {
        /** Whether the Session's current shares still allow writing it. */
        readonly mayWrite: () => boolean;
    };
    /** Refresh the Session's Relay grants before an operation that consumes its filesystem capability. */
    readonly refreshAuthorization?: () => Promise<void>;
    /**
     * Whether one execution belongs to a turn whose entering input named a share viewer.
     *
     * The layer belongs to the Session, but it confines only the turns a viewer started: a turn the owner
     * started keeps the deployment's own tools, including its own path resolution, so the answer is read
     * for every execution while the layer stays mounted. Absent means every execution is confined, which
     * keeps a host that cannot attribute turns from widening one.
     */
    readonly confinedExecution?: (execution: ForkToolExecution) => boolean;
}
/**
 * Confine every viewer-started turn of one shared Session to its granted workspace.
 *
 * The replacements are registered in the Session's scope, and while a viewer-started turn runs, the guard
 * on that scope permits nothing else: that turn reads through the confined replacements, and where the
 * Session's shares allow writing it writes through the bounded ones. The owner's own turns ask the same
 * guard but keep the deployment's tools, so the share never narrows the work the owner started. Every
 * path a permitted viewer turn declares is resolved and checked against the granted root before the
 * deployment's body receives its canonical form, and each call re-reads the shares' current Relay grants,
 * so a share that narrows or ends stops its own writes without the layer being reinstalled. A tool the
 * shared surface does not name, or one whose replacement could not register, is refused to that turn.
 * Stopping the layer is what restores the Session's own tools, which the host does when no share remains.
 * @param target - The Session, its granted workspace, and the authority that answers per operation.
 * @returns The installation: its disposer, and whether reads and writes can actually be confined here.
 */
export declare function installSourceSessionAccess(target: SourceSessionAccess): SourceSessionAccessInstallation;
/** Path resolution and text writes as the filesystem capability exposes them. */
export interface ForkEnforcementFs {
    readonly resolve: (path: string, options?: {
        readonly cwd?: string;
        readonly signal?: AbortSignal;
    }) => Promise<FsTarget>;
    readonly contains: (parent: FsTarget, child: FsTarget) => boolean;
    /**
     * The canonical path a subprocess in this filesystem's world opens for a target.
     *
     * Delegating with this value hands the deployment's tool the path this host just checked instead of
     * the model's own spelling.
     */
    readonly processPath: (target: FsTarget) => string;
    /**
     * Write text to one resolved target under the Session's complete policy.
     *
     * `expected` carries the caller's intent — a version to replace, or a requirement that the file is
     * absent — so the deployment's guarded-write semantics hold. `sandboxPolicy` is resolved after the
     * Session's write-intent listeners settle; a sandboxing backend fences the write by it.
     */
    readonly writeText: (target: FsTarget, content: string, expected?: FsWriteIntent, signal?: AbortSignal, sandboxPolicy?: SandboxExecutionPolicy) => Promise<FsWriteOutcome>;
    /**
     * Edit one resolved target literally under the Session's complete policy.
     *
     * `expected` carries the caller's edit intent — the version the caller observed — so the deployment's
     * guarded-edit semantics hold. `sandboxPolicy` is resolved after the Session's edit-intent listeners
     * settle; a sandboxing backend fences the edit by it.
     */
    readonly editText: (target: FsTarget, edit: FsEditRequest, expected?: {
        readonly version: FsVersion;
    }, signal?: AbortSignal, sandboxPolicy?: SandboxExecutionPolicy) => Promise<FsEditOutcome>;
}
/** The tool registry's definition lookup, as the tool runtime exposes it. */
export interface ForkToolRegistry {
    readonly get: (name: string, scope?: object) => ToolDefinition | undefined;
}
/** Paths one tool's own arguments name, or undefined when they cannot be established. */
export interface ForkToolPaths {
    readonly paths: readonly string[] | undefined;
    readonly noFileEffect: boolean;
}
/** Everything one child's limit is installed from, once the host has resolved its own services. */
export interface ForkEnforcementTarget {
    readonly childSessionId: string;
    /** The child's own scoped context, which the guard and the verdict producer register into. */
    readonly childContext: Context;
    /**
     * The child's own scope key.
     *
     * A guard registered from an unscoped context would apply to every agent in the process, so the
     * host derives this from the child's context and refuses to install without one.
     */
    readonly childScope: object;
    /**
     * Canonical root of the workspace this child was granted, resolved once when the limit was
     * installed.
     *
     * Resolving it again per read would re-derive the grant from a path that may point elsewhere by
     * then, so the limit carries the root it was installed with.
     */
    readonly grantedRoot: FsTarget;
    /** Working directory the child's relative paths resolve against. */
    readonly cwd: string;
    /** Resolve this Session's complete current policy after write-intent listeners settle. */
    readonly resolvePolicy: () => SandboxExecutionPolicy;
    readonly fs: ForkEnforcementFs;
    /** The deployment's own tool registry, which decides whether a name is trusted. */
    readonly tools: ForkToolRegistry;
    readonly forks: ForkRegistry;
    readonly describe: (execution: ForkToolExecution) => ForkToolPaths;
    /** Whether this scope may also write, through a `write` tool this host installs itself. */
    readonly write?: boolean;
    /** Re-check the share's current grant immediately before each tool execution. */
    readonly authorized?: (tool: string) => boolean;
    /** Refresh the child's current Relay grant before consuming its filesystem capability. */
    readonly refreshAuthorized?: () => Promise<void>;
}
/** Host services and arguments one host-side installation reads. */
export interface ForkEnforcementHost {
    readonly forks: ForkRegistry;
    readonly describe: (execution: ForkToolExecution) => ForkToolPaths;
    /**
     * Whether the share this child serves may write, which puts this host's bounded write in the child's
     * scope.
     *
     * Read from the owner's published share rather than from configuration: a child must never install a
     * wider ceiling than the share its viewer was admitted to, and absent means reading only.
     */
    readonly write?: boolean;
    /** Re-check the share's current grant immediately before each tool execution. */
    readonly authorized?: (tool: string) => boolean;
    /** Refresh the child's current Relay grant before consuming its filesystem capability. */
    readonly refreshAuthorized?: () => Promise<void>;
}
/**
 * What one installation could not do, for a caller that states the refusal itself.
 *
 * Installation stays fail-closed: a replacement that cannot register returns undefined rather than
 * throwing, so a child is never published under a ceiling this host did not install. The error the scope
 * raised is kept here because the caller's own refusal is the only place it can be reported.
 */
export interface ForkEnforcementFailure {
    /** The confined tool whose registration the child's scope refused. */
    registration?: ForkToolRegistrationFailure;
}
/**
 * Prove one recorded child's limit can be installed before its agent is published.
 *
 * A resumed child is announced through `agent/created` before its limit installs, so an installability
 * failure would leave a created Session whose guard never landed. Reading the stored header and resolving
 * the workspace it names happens before activation, so the child is never announced when the limit cannot
 * land. A Session whose header cannot be read fails here as well: this host cannot prove the limit
 * installs, and the caller refuses the child instead of activating one it could not confine. A
 * composition without a session query leaves the child to activation, because there is no header to read.
 * @param ctx - Host context carrying the session query and filesystem.
 * @param childSessionId - Recorded fork child to preflight.
 * @returns Nothing when the child's granted workspace resolves.
 * @throws when the child's Session cannot be read, states no workspace, or its workspace cannot be
 *   resolved.
 */
export declare function preflightForkChild(ctx: Context, childSessionId: string): Promise<void>;
/**
 * Install one child's limit against the host's own services.
 *
 * Every dependency is checked before anything registers: the record this writes is what opens the
 * child's prompt gate, so a host missing its agent registry, filesystem, sandbox policy, or live child
 * must leave that child refused rather than report a limit nothing applies.
 * @param ctx Host context that provides the agent registry, filesystem, sandbox policy, and tools.
 * @param childSessionId Child that was just forked.
 * @param host The host's fork registry and argument reader.
 * @param failure Receives the error a refused registration raised, so the caller can state it.
 * @returns The disposer that releases this child's limit, or undefined when nothing was installed.
 */
export declare function installHostForkEnforcement(ctx: Context, childSessionId: string, host: ForkEnforcementHost, failure?: ForkEnforcementFailure): Promise<(() => void) | undefined>;
/**
 * Install one child's limit from the scope the factory hands its setup.
 *
 * A resumed agent is unpublished while its setup runs, so the registry cannot find it yet: the caller
 * passes that setup's own context and session instead of publishing first to make a lookup succeed.
 * @param ctx Host context that provides the filesystem, sandbox policy, and tools.
 * @param childSessionId Child this scope belongs to.
 * @param childContext The unpublished child's own scope.
 * @param session The child's session, whose granted workspace the limit is confined to.
 * @param host The host's fork registry and argument reader.
 * @param failure Receives the error a refused registration raised, so the caller can state it.
 * @returns The disposer that releases this child's limit, or undefined when nothing was installed.
 */
export declare function installForkEnforcementForAgent(ctx: Context, childSessionId: string, childContext: Context, session: Session, host: ForkEnforcementHost, failure?: ForkEnforcementFailure): Promise<(() => void) | undefined>;
/**
 * Install one child's limit and register the parts that make it real.
 * @param target The child's own context and every resolved capability the limit needs.
 * @param failure Receives the error a refused registration raised, for the caller's refusal.
 * @returns The disposer that releases the limit, or undefined when nothing was installed.
 */
export declare function installForkEnforcement(target: ForkEnforcementTarget, failure?: ForkEnforcementFailure): (() => void) | undefined;
//# sourceMappingURL=fork-enforcement.d.ts.map