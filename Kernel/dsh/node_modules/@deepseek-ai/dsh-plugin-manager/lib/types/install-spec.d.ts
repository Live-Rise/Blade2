/**
 * Reading an install spec before pnpm sees it: which of pnpm's spec forms it
 * takes, for a registry name whether it is one the registry can accept, and
 * for a Git spec pinned to a commit the repository directory it installs from.
 * @module @deepseek-ai/dsh-plugin-manager/install-spec
 */
/**
 * One spec read into its form and the parts an inspection needs. A git spec
 * and a tarball URL carry the `host` pnpm fetches them from, which no registry
 * stands in for; only their dependencies come from the registry.
 */
export type ParsedInstallSpec = {
    readonly kind: 'registry';
    readonly spec: string;
    readonly name: string;
    readonly range?: string;
} | {
    readonly kind: 'path';
    readonly spec: string;
    readonly path: string;
} | {
    readonly kind: 'tarball';
    readonly spec: string;
    readonly path?: string;
    readonly host?: string;
} | {
    readonly kind: 'git';
    readonly spec: string;
    readonly host: string;
};
/** A spec neither pnpm nor the registry would take; `reason` is what the person reads. */
export declare class InvalidInstallSpecError extends Error {
    readonly spec: string;
    readonly reason: string;
    /**
     * @param spec - the spec as typed, trimmed.
     * @param reason - why it is refused, as one sentence.
     */
    constructor(spec: string, reason: string);
}
/**
 * Read a spec into its form. A path must be absolute: the Host's working
 * directory means nothing to the person typing into a browser, and a
 * relative path resolved against the profile would point inside it.
 * @param raw - the spec as typed.
 * @returns the parsed spec.
 * @throws {InvalidInstallSpecError} for an empty spec, a relative path, a name the
 * registry would refuse, or a URL that is neither a git host nor a tarball.
 */
export declare function parseInstallSpec(raw: string): ParsedInstallSpec;
/**
 * Where a Git spec pinned to a full commit installs from: the lower-case host and the repository path, without
 * transport, user, or `.git`, then `#` and the `path:` directory. The origin ignores the form pnpm saves, so the SSH
 * form saved for an HTTPS repository it cannot read, and the host shortcut saved for a known host, have the HTTPS
 * form's origin. Equal origins name one repository on a host that serves it at the same path over every transport,
 * as Git forges do; on a plain SSH server, an scp-like path is relative to the login's home directory.
 * @param spec - an install spec, such as a saved dependency or an update's target.
 * @returns the origin, or undefined for a spec that is not Git, or whose fragment is not one full commit and at most
 * one `path:` directory.
 */
export declare function pinnedGitOrigin(spec: string): string | undefined;
/** The user information of an http(s) URL, up to the last `@` before the path, which holds a password or access token when present. */
export declare const HTTP_USER_INFO: RegExp;
/**
 * The spec `pnpm add` accepts for a dependency the profile manifest records. A git address, URL, or other
 * non-registry protocol is the recorded value without the user information of an http(s) URL, whose query string
 * is kept; a `file:` or `link:` path, which pnpm records relative to the profile or with `~` for the home
 * directory, becomes absolute. A registry range, tag, or alias follows the dependency name after `@`, as does a
 * non-registry value installed under a name other than the package's own.
 * @param name - the dependency name.
 * @param recorded - the value the profile manifest records for it.
 * @param profileDir - the profile directory a relative local path is resolved against.
 * @param packageName - the name the installed package's manifest declares, when it could be read.
 * @returns the installable spec.
 */
export declare function dependencySpec(name: string, recorded: string, profileDir: string, packageName?: string): string;
//# sourceMappingURL=install-spec.d.ts.map