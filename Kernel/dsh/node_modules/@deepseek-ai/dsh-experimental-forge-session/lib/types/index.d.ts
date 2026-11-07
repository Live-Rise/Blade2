/** Binds change requests to the Session that created, pushed, or tracked them, and watches the bound ones. */
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Session } from '@deepseek-ai/dsh-session';
import type { Config, SessionChangeRequest, SessionChangeRequestsListener } from './types.ts';
export type * from './types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        forgeSession: ForgeSessionService;
    }
}
/**
 * Session-scoped change request bindings. The `forgeChangeRequests`
 * projection derives them from `gh pr create`, `gh pr view`, `gh pr edit`,
 * `gh pr checkout`, and `git push` results and from
 * `plugin:forge-session/binding` records. After each push result, and when a
 * caller reports the branch a Session works on, the service looks up open
 * change requests whose head is that branch and records the unbound ones as
 * discovered. Watching a Session polls its visible change requests through
 * `ctx.forge`.
 */
export declare class ForgeSessionService extends Service {
    static inject: string[];
    static Config: z<Config>;
    private readonly home;
    private readonly discoveryTimeoutMs;
    private readonly watches;
    private readonly discoveries;
    private readonly sessionLifetimes;
    private readonly lifetime;
    private projection;
    constructor(ctx: Context, config: Config);
    /**
     * List the change requests bound to a Session and not dismissed.
     * @param session - Session whose bindings are read.
     * @returns visible bindings in first-recorded order, each with its latest polling result.
     */
    changeRequests(session: Session): SessionChangeRequest[];
    /**
     * Bind a change request to a Session by URL, or restore a dismissed binding.
     * Appends a `track` record with the provider's canonical URL unless the
     * change request is already visible.
     * @param session - Session that receives the binding.
     * @param url - web URL of the change request.
     * @throws when no registered provider recognizes the URL.
     */
    track(session: Session, url: string): void;
    /**
     * Look up open change requests whose head is a branch the Session works on,
     * and record the unbound ones as discovered, as after a push of that branch.
     * A remote that no provider recognizes changes nothing; a failed lookup is
     * logged and changes nothing.
     * @param session - Session that receives the bindings.
     * @param remote - URL of the remote that holds the branch.
     * @param branch - branch name on that remote.
     * @returns resolves once the lookup has recorded its bindings or failed.
     */
    discoverBranch(session: Session, remote: string, branch: string): Promise<void>;
    /**
     * Hide a visible binding. Appends a `dismiss` record; a URL that is not
     * visibly bound leaves the log unchanged. Discovery never restores a
     * dismissed binding; `track` does.
     * @param session - Session that owns the binding.
     * @param url - web URL of the change request.
     */
    dismiss(session: Session, url: string): void;
    /**
     * Watch a Session's visible change requests as an effect on the calling
     * fiber. Every visible change request that a provider recognizes is polled
     * through `ctx.forge` while the Session has a watcher. Listeners receive the
     * complete list after each later binding or provider change, or polling result; read the
     * current list with {@link changeRequests}. Listeners must not throw; a
     * thrown error is logged and skipped.
     * @param session - Session to watch.
     * @param listener - receives the complete visible list.
     * @returns the disposer that stops this watch.
     */
    watch(session: Session, listener: SessionChangeRequestsListener): () => void;
    protected [Service.init](): Promise<void>;
    private state;
    private entries;
    /** Discover change requests for this result's pushes and refresh watchers after binding changes. */
    private react;
    /** Poll exactly the visible change requests that a provider recognizes. */
    private sync;
    private notify;
    private discover;
    private lookup;
}
export default ForgeSessionService;
//# sourceMappingURL=index.d.ts.map