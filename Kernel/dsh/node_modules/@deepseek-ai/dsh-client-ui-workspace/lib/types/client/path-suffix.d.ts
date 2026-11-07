/** Path suffixes that tell same-titled Workspaces apart. */
/**
 * Map each Workspace whose display title another listed Workspace shares to
 * the shortest run of trailing path segments that no other same-titled
 * Workspace's path ends with, keeping the path's own separators. Workspaces
 * with a unique title get no entry.
 * @param workspaces - id, display title, and canonical path of each listed Workspace.
 * @returns distinguishing suffix by Workspace id.
 */
export declare function distinguishingPathSuffixes<Id>(workspaces: readonly {
    readonly id: Id;
    readonly title: string;
    readonly path: string;
}[]): ReadonlyMap<Id, string>;
//# sourceMappingURL=path-suffix.d.ts.map