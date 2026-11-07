/** The fields a version listing prints for the package: every version, its publication times, and its dist-tags. */
export declare const VERSION_LIST_FIELDS: readonly ["versions", "time", "dist-tags"];
/** The fields read for one version: what decides whether it can replace an installed bundle. */
export declare const VERSION_FIELDS: readonly ["name", "version", "engines", "peerDependencies", "deprecated", "dsh"];
/** One published version newer than the installed one, before its own manifest is read. */
export interface PublishedVersion {
    readonly version: string;
    readonly publishedAt?: string;
    readonly tags: readonly string[];
    readonly prerelease: boolean;
}
/** What one version's manifest says about replacing an installed bundle. */
export interface VersionFacts {
    /** The fields `evaluatePluginCompatibility` reads. */
    readonly manifest: {
        name: string;
        version: string;
        engines?: unknown;
        peerDependencies?: unknown;
    };
    readonly bundle: boolean;
    readonly deprecated?: string;
}
/**
 * The published versions newer than the installed one, newest first.
 * @param answer - parsed `pnpm view <name> versions time dist-tags --json` output.
 * @param installed - the installed version.
 * @returns the newer versions with their publication times and dist-tags.
 * @throws when the answer names no versions.
 */
export declare function newerVersions(answer: unknown, installed: string): PublishedVersion[];
/**
 * What one exact version's manifest says, from `pnpm view <name>@<version> name version engines peerDependencies deprecated dsh --json`.
 * @param answer - the parsed output.
 * @param name - the package name, used when the answer omits it.
 * @param version - the version asked for, used when the answer omits it.
 * @returns the manifest fields compatibility reads, whether the version declares a bundle, and its deprecation message.
 */
export declare function versionFacts(answer: unknown, name: string, version: string): VersionFacts;
//# sourceMappingURL=versions.d.ts.map