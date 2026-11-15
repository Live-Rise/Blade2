/**
 * Session working directories, durable changes, and model context.
 * @module @deepseek-ai/dsh-working-directory
 */
import { Context, Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { Session } from '@deepseek-ai/dsh-session';
export { WorkingDirectoryError, type WorkingDirectoryErrorCode } from './error.ts';
declare module '@deepseek-ai/cordis' {
    interface Events {
        /**
         * Coordinate directory validation and publication with a resource operation
         * on the directory, such as a checkout being removed. A listener that must
         * wait awaits that operation, then calls `next()`; every other listener
         * calls `next()` at once. A rejection fails this access and leaves the
         * recorded directory unchanged; it is not a fallback.
         * @param request - owner, requested path (null validates the current directory), and the access's cancellation.
         * @param next - perform validation and commit.
         * @mode waterfall
         */
        'working-directory/access'(request: {
            agent: Agent;
            path: string | null;
            signal: AbortSignal;
        }, next: () => Promise<string>): Promise<string>;
    }
    interface Context {
        workingDirectory: WorkingDirectoryService;
    }
}
declare module '@deepseek-ai/dsh-llm' {
    interface MessageSourceMap {
        /**
         * Committed working-directory transition notices.
         * @persistenceAttribution
         */
        'working-directory': {
            kind: 'working-directory';
        };
    }
}
declare module '@deepseek-ai/dsh-session/types' {
    interface SessionEventMap {
        /** Effective working directory; the immutable Session header retains the original project. */
        'working-directory/change': {
            /** Absolute directory used by subsequent directory-based operations. */
            cwd: string;
        };
    }
}
declare module '@deepseek-ai/dsh-session-projection/types' {
    interface SessionProjectionMap {
        /** Recorded current directory for Session observations, or null when none is recorded. */
        workingDirectory: string | null;
    }
}
/** Deployment default for Sessions without an original project directory. */
export interface Config {
    /** Absolute execution-world fallback directory; omitted values use the launch directory. */
    defaultDirectory?: string;
}
/** One owner for each Session's effective directory and its model-visible changes. */
export declare class WorkingDirectoryService extends Service {
    static inject: string[];
    static Config: z<Config>;
    /** Absolute deployment fallback for Sessions with no recorded original directory. */
    readonly defaultDirectory: string;
    private readonly pending;
    private readonly operations;
    private readonly lifecycle;
    private workingDirectoryProjectionHandle;
    constructor(ctx: Context, config: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Read the committed directory without filesystem I/O.
     * @param session - Session whose directory is requested.
     * @returns its effective absolute directory.
     */
    get(session: Session): string;
    /**
     * Validate the current directory and restore the original project when it disappeared.
     * @param agent - live or unpublished Agent owning the Session.
     * @param signal - cancellation for filesystem inspection.
     * @returns the existing directory; recovery is committed before fulfillment.
     * A notice failure is warned without reverting the committed state.
     * @throws {@link WorkingDirectoryError} `WORKING_DIRECTORY_UNAVAILABLE` when the original project is also
     *   unavailable or the service is disposed, or the rejection of a `working-directory/access` listener.
     */
    ensure(agent: Agent, signal?: AbortSignal): Promise<string>;
    /**
     * Change one Session's directory without changing existing processes or permissions.
     * @param agent - live or unpublished Agent owning the Session.
     * @param path - absolute path or a path relative to its current directory.
     * @param signal - cancellation before the durable change.
     * @returns the canonical absolute directory, committed before fulfillment.
     * A notice failure is warned; the next request still receives the committed directory.
     * @throws {@link WorkingDirectoryError} `WORKING_DIRECTORY_INVALID` for an empty path and
     *   `WORKING_DIRECTORY_NOT_FOUND` when the path is not an existing directory, or the rejection of a
     *   `working-directory/access` listener.
     */
    set(agent: Agent, path: string, signal?: AbortSignal): Promise<string>;
    private contextText;
    private directory;
    private commit;
    private serialize;
}
export default WorkingDirectoryService;
//# sourceMappingURL=index.d.ts.map