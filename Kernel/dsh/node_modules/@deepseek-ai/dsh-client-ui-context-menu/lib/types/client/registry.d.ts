import type { MenuContribution, MenuResolution, ContextMenuTarget, ContextMenuTargetMap } from './types.ts';
/** React-free registry; each occurrence captures the providers registered at opening. */
export declare class MenuRegistry {
    private readonly failed;
    private readonly timeoutMs?;
    private readonly providers;
    private readonly occurrences;
    private disposed;
    constructor(failed: (message?: string) => void, timeoutMs?: number | undefined);
    /**
     * Register actions for subsequent openings.
     * @param input - provider definition.
     * @returns removal of this contribution and its pending resolutions.
     */
    register<K extends keyof ContextMenuTargetMap>(input: MenuContribution<K>): () => void;
    /**
     * Resolve an independently owned occurrence.
     * @param target - captured clicked identity.
     * @returns presentation, selection, and disposal.
     */
    resolve(target: ContextMenuTarget): MenuResolution;
    /** Cancel every occurrence and reject further registrations or resolutions. */
    dispose(): void;
}
//# sourceMappingURL=registry.d.ts.map