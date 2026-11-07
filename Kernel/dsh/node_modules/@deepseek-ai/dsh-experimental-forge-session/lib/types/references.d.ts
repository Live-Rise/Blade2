/** Literal GitHub CLI references, with shell quoting and command separators preserved. */
/** One PR explicitly selected by a supported GitHub CLI invocation. */
export interface GhReference {
    /** Canonical URL when the command supplies its host, otherwise null. */
    readonly url: string | null;
    /** Read-only command for an unqualified repository in the first simple command, otherwise null. */
    readonly lookup: string | null;
}
/**
 * Read explicit URL selectors or numeric selectors with `--repo`/`-R`.
 * Shell expansion, multiline scripts, substitutions, and unsupported options
 * are excluded. Unqualified repositories can be resolved only for a single
 * simple command, optionally piped to an output filter; compound commands
 * may change the CLI environment. PowerShell callers may use only `url`.
 * @param command - logged shell command text.
 * @returns references in command order; no process or environment is read.
 */
export declare function ghReferences(command: string): GhReference[];
//# sourceMappingURL=references.d.ts.map