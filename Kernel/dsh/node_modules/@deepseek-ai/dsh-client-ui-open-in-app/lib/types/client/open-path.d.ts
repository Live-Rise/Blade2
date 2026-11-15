/**
 * Host desktop availability, file associations, and open/reveal actions over the Session Remote.
 * The desktop answer is read once per page; a failed read renders no control. Association answers
 * are recorded per association class so menus can show them without waiting for the Host.
 */
import { type ObservableSnapshot, type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { RemoteResult } from '@deepseek-ai/dsh-api-remotes/client';
import type { SessionOpenWorkspacePathRequest, SessionWorkspacePathApplication } from '@deepseek-ai/dsh-api-session-controller/types';
import type { OpenInAppOutcome } from './outcome.ts';
type Applications = readonly SessionWorkspacePathApplication[];
/** What a path gesture asks of the Host desktop: the default application, or the file manager showing the file. */
export type OpenInAppPathAction = 'open' | 'reveal';
/** The slice of the Client Remote the path actions call. */
export interface OpenInAppPathRemote {
    readonly session: {
        /** Whether the Host can hand a workspace path to a native desktop. */
        canOpenWorkspacePath(): Promise<RemoteResult<boolean>>;
        /** Current registered file handlers and their default selection. */
        workspacePathApplications(request: {
            path: string;
        }, signal?: AbortSignal): Promise<RemoteResult<readonly SessionWorkspacePathApplication[]>>;
        /** Open one Host path in its default application, or reveal it in the file manager. */
        openWorkspacePath(request: SessionOpenWorkspacePathRequest, signal?: AbortSignal): Promise<RemoteResult<unknown>>;
    };
}
/**
 * Page-lifetime desktop availability, association answers, and the open/reveal carrier shared by every path control.
 * Answers are kept per association class for the page's life and grow with the classes queried.
 */
export declare class OpenInAppPathController {
    private readonly remote;
    /** Whether the Host desktop can open paths; null until the Host answered, false also after a failed read. */
    readonly desktop: SnapshotStore<boolean | null>;
    private readonly answers;
    /** Latest successful answer per association class; the snapshot keeps its identity until an answer changes. */
    readonly associations: ObservableSnapshot<ReadonlyMap<string, Applications>>;
    private loading;
    private readonly lifetime;
    /** The latest background refresh; a newer refresh aborts it. */
    private refresh;
    /** Start order of the query whose answer each class holds; an earlier query never replaces a later one. */
    private readonly answeredBy;
    /** One icon string per application id, reused by every class that lists the application so its data URL is held once. */
    private readonly icons;
    private queries;
    /**
     * @param remote - the Session Remote namespace answering availability and running gestures.
     */
    constructor(remote: OpenInAppPathRemote);
    /**
     * Read desktop availability once per controller life; concurrent calls share the read.
     * @returns after availability is published.
     */
    load(): Promise<void>;
    /**
     * Open one Host path in its default application, or reveal it in the file manager.
     * @param path - absolute path on the Host, as the file's metadata reports it.
     * @param action - default application open, or file-manager reveal.
     * @param application - registered application path for an explicit open.
     * @returns the acknowledged open or reveal, or the named failure to announce.
     */
    openPath(path: string, action: OpenInAppPathAction, application?: string): Promise<OpenInAppOutcome>;
    /**
     * Query the file's current OS associations and record a successful answer for its association class.
     * Failures remain distinct from an empty handler list and leave the recorded answer in place.
     * @param path - file path reported by the Host.
     * @param signal - lifetime of the requesting control.
     * @returns application metadata, or null when the query fails.
     */
    applications(path: string, signal: AbortSignal): Promise<Applications | null>;
    /**
     * Read the recorded answer of a file's association class without querying the Host.
     * @param path - absolute Host path of the file.
     * @returns the latest successful answer, or undefined before a query of this class succeeded.
     */
    cachedApplications(path: string): Applications | undefined;
    /**
     * Re-query a file's associations in the background. One refresh runs at a time: it outlives the
     * requesting menu and ends with its answer, the next refresh, which cancels it, or {@link dispose}.
     * @param path - absolute Host path of the file.
     * @returns settles without rejecting when this refresh's query settles, after any answer is recorded;
     * already settled after {@link dispose}.
     */
    refreshApplications(path: string): Promise<void>;
    /** Cancel background refreshes and refuse new ones; recorded answers stay readable. */
    dispose(): void;
    private record;
    private run;
}
export {};
//# sourceMappingURL=open-path.d.ts.map