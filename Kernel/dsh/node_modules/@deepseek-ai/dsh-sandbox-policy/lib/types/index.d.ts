/**
 * The sandbox POLICY home (`ctx.sandboxPolicy`): the single owner of the
 * deployment's sandbox fallbacks plus per-session resolution: the file-effect
 * {@link SandboxMode}, the `workspace-write` root, the explicit extra writable
 * directories from configuration and live Session registrations, and the override
 * kit (the `sandbox/mode` event, its fold, and its write path; the fold is the
 * `sandboxMode` session-projection unit
 * registered here, while the event and write path come from
 * `./session-mode.ts`).
 * Before each agent request, the owner also contributes the resolved policy to
 * the cache-safe runtime-context snapshot. The agent loop logs that snapshot as
 * model history, so replay reconstructs the same mode and root the enforcing
 * consumers resolve without rewriting the stable system prompt.
 *
 * Enforcing filesystem, one-shot bash, and terminal backends read the SAME
 * resolved policy here. The context describes that policy without inventorying
 * capabilities, while each backend retains its own enforcement dialect and each
 * tool owns its operation-specific denial and escalation guidance. The service
 * reads session state once at each operation boundary; executors and providers
 * remain session-free.
 *
 * @module @deepseek-ai/dsh-sandbox-policy
 */
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { SandboxExecutionPolicy, SandboxMode } from '@deepseek-ai/dsh-sandbox';
import type { Session } from '@deepseek-ai/dsh-session';
export { SANDBOX_MODES, setSandboxMode } from './session-mode.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        sandboxPolicy: SandboxPolicyService;
    }
}
/**
 * Plugin config: the deployment's sandbox default. All optional — `Config`
 * supplies the defaults (`mode: 'read-only'` is the fail-safe default; a
 * deployment that wants a workspace-writable agent opts in explicitly). The
 * runner choice is NOT here (it is the `ctx.sandbox` provider's config), nor
 * is any per-family knob: this is the one shared policy home.
 */
export interface Config {
    /** File-sandbox mode a session starts from (default: `read-only`). */
    mode?: SandboxMode;
    /**
     * Absolute fallback root for agentless calls and sessions without a cwd (default:
     * `process.cwd()`). Normal agent calls use their session cwd instead.
     */
    workspaceRoot?: string;
    /**
     * Extra physical absolute directories that `workspace-write` may also write under
     * (default: none). The supervisor names them explicitly; there is no
     * automatic repository or gitfile discovery. Entries are execution-world
     * paths: this service validates absoluteness and detaches the list, while
     * the enforcing host rejects missing, non-directory, or symlinked paths. They grant nothing under
     * `read-only`. Windows process confinement ignores them; harness file tools honor them.
     */
    additionalWritableRoots?: string[];
}
/** Inputs that select the sandbox policy for one capability call. */
export interface SandboxPolicyRequest {
    /** Calling session; its immutable cwd becomes the workspace boundary. */
    session?: Session;
    /** Explicit approved mode override, which outranks session policy. */
    mode?: SandboxMode;
}
/**
 * The sandbox-policy service (`ctx.sandboxPolicy`). Owns the deployment
 * default mode, fallback workspace root, session-scoped directory grants, and
 * request-time policy section. Tool layers call {@link resolve} for each execution so a session's
 * mode log and immutable cwd travel together to every enforcing capability.
 */
export declare class SandboxPolicyService extends Service {
    static Config: z<Config>;
    static inject: string[];
    /** The deployment default mode — the fallback beneath a session override. */
    readonly defaultMode: SandboxMode;
    /** The absolute `workspace-write` fallback root for calls without a session cwd. */
    readonly workspaceRoot: string;
    /** The frozen extra writable directories shared by every resolved policy, or `undefined` for none. */
    readonly additionalWritableRoots: readonly string[] | undefined;
    /** Independently owned grants for each live Session; never restored from its log. */
    private readonly writableRootRegistrations;
    private sandboxModeProjectionHandle;
    constructor(ctx: Context, config: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Resolve the complete policy for one capability call. An approved explicit
     * mode outranks the session's last `sandbox/mode` event, which outranks the
     * deployment default. A session cwd is its workspace-write boundary; the
     * configured root is the fallback for agentless calls and sessions without a
     * cwd. Configured extra roots and this Session's active registrations are
     * snapshotted together; existing operations and processes keep their policy.
     * @param request - optional session and approved mode override.
     * @returns the fully resolved per-call mode and absolute workspace root.
     * @throws `SandboxInvalidRootError` (`SANDBOX_INVALID_ROOT`) when the session's recorded cwd is
     *   relative or contains a NUL byte.
     */
    resolve(request?: SandboxPolicyRequest): SandboxExecutionPolicy;
    /** The session's recorded cwd, or the configured root, as the workspace-write boundary. */
    private sessionWorkspaceRoot;
    /**
     * Permit writes under one directory in this Session's future workspace-write
     * operations. The calling context owns the effect; disposal removes only this
     * registration. Other grants to the directory remain effective. Existing
     * operations and persistent processes retain their resolved permissions.
     * Grants are runtime-only and must be registered again after resume or fork.
     * On Windows, only harness file tools honor extra grants; process permissions stay unchanged.
     * @param session - exact live Session receiving the grant; other Sessions and agentless calls are unaffected.
     * @param directory - absolute execution-world path; execution rejects missing, non-directory, or symlinked paths.
     * @returns the exact Cordis effect disposer for this registration.
     * @throws when the directory is relative or contains a NUL byte.
     */
    registerWritableRoot(session: Session, directory: string): () => void;
    /**
     * Read the session override without applying the deployment default.
     * @param session - session whose log supplies the override.
     * @returns the last logged mode, or `undefined` without one.
     */
    overrideOf(session: Session): SandboxMode | undefined;
}
export default SandboxPolicyService;
//# sourceMappingURL=index.d.ts.map