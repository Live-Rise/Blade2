/**
 * Search exclusion rules and the search preferences that carry them.
 *
 * A rule names files and directories the workspace search leaves out, by name
 * or by path. A path without `/` is a name and matches every file or directory
 * of that name at any depth; a path with `/` is relative to the workspace root
 * and matches that one file or directory. An excluded directory's contents are
 * left out with it. This module keeps the editable list, checks what the
 * reader types, and reads the list back from a stored value; the Host's
 * workspace search applies the rules.
 *
 * The list records no origin: a default path the reader removed and then
 * added again by hand counts as a default that is present.
 */
/** One exclusion rule as the settings list shows and stores it. */
export interface ExclusionRule {
    /** Normalized name or `/`-separated workspace-relative path of a file or directory. */
    readonly path: string;
    /** Whether the search leaves this path out now. */
    readonly enabled: boolean;
}
/** Why a typed rule path was refused; each reason has its own `settings.invalid.*` line. */
export type RuleInvalidReason = 'empty' | 'nul' | 'absolute' | 'parent' | 'glob';
/** A typed rule path after normalization, or why it was refused. */
export type RulePathResult = {
    readonly kind: 'ok';
    readonly path: string;
} | {
    readonly kind: 'invalid';
    readonly reason: RuleInvalidReason;
};
/** The search panel's persisted choices. */
export interface WorkspaceSearchPreferences {
    /** Exclusion rules, newest first. */
    readonly rules: readonly ExclusionRule[];
    /** Whether results include hidden entries. */
    readonly showHidden: boolean;
    /** Whether the search leaves out entries the workspace's `.gitignore` files ignore. */
    readonly respectGitignore: boolean;
}
/**
 * Normalize a typed rule path: trim it, turn `\` into `/`, merge repeated
 * `/`, and drop leading `./`, trailing `/`, and whitespace exposed at either
 * edge. Internal whitespace stays unchanged.
 *
 * Refuses an empty path, one containing NUL, an absolute one (leading `/`, a
 * drive letter such as `C:`, or `~`, including prefixes exposed by dropping
 * leading `./` and whitespace), one with a `.` or `..` segment, and one
 * containing a glob character (`* ? [ ] { } !`). Other characters, such as
 * `$` and spaces, are kept literally.
 * @param input - the text the reader typed.
 * @returns the normalized path, unchanged by normalization again, or why it was refused.
 */
export declare function normalizeRulePath(input: string): RulePathResult;
/**
 * The default rule list, every rule enabled.
 * @returns a new list in default order.
 */
export declare function defaultRules(): ExclusionRule[];
/**
 * The preferences before the reader changes anything.
 * @returns the default rules, hidden entries not shown, and `.gitignore` respected.
 */
export declare function defaultSearchPreferences(): WorkspaceSearchPreferences;
/**
 * Add a typed rule at the top of the list, enabled.
 *
 * A rule whose normalized path is already listed, enabled or not, is enabled
 * and moved to the top instead of being listed twice. Paths compare exactly.
 * @param rules - the current list.
 * @param input - the text the reader typed.
 * @returns the new list, or why the text was refused.
 */
export declare function addRule(rules: readonly ExclusionRule[], input: string): {
    readonly kind: 'ok';
    readonly rules: readonly ExclusionRule[];
} | {
    readonly kind: 'invalid';
    readonly reason: RuleInvalidReason;
};
/**
 * Remove one rule.
 * @param rules - the current list.
 * @param path - the rule's normalized path.
 * @returns the list without it; an unlisted path leaves the list as it was.
 */
export declare function removeRule(rules: readonly ExclusionRule[], path: string): readonly ExclusionRule[];
/**
 * Enable or disable one rule in place.
 * @param rules - the current list.
 * @param path - the rule's normalized path.
 * @param enabled - whether the search leaves the path out.
 * @returns the list with that rule changed; an unlisted path leaves the list as it was.
 */
export declare function setRuleEnabled(rules: readonly ExclusionRule[], path: string, enabled: boolean): readonly ExclusionRule[];
/**
 * The paths the search leaves out.
 * @param rules - the current list.
 * @returns the enabled rules' paths, in list order.
 */
export declare function enabledPaths(rules: readonly ExclusionRule[]): string[];
/**
 * Whether a rule list equals the default list: the same paths in the same
 * order, every rule enabled.
 * @param rules - the current list.
 * @returns `true` when nothing differs from {@link defaultRules}.
 */
export declare function isDefaultRules(rules: readonly ExclusionRule[]): boolean;
/**
 * Read preferences back from a value browser storage holds, which any earlier
 * build or the reader may have written, after its JSON text was parsed.
 *
 * Each field is read on its own: a missing or mistyped `rules` array,
 * `showHidden`, or `respectGitignore` takes the current default, so an entry
 * that stores only the fields the reader changed keeps the defaults of the
 * running build for the rest. A value that is not an object gives the
 * defaults. Inside a `rules` array, a rule that is not an object with a string
 * `path` and a boolean `enabled`, or whose path `normalizeRulePath` refuses,
 * is dropped; a rule whose normalized path repeats an earlier one is dropped
 * too. A stored empty list stays empty.
 * @param value - the parsed stored value.
 * @returns the preferences to use.
 */
export declare function readSearchPreferences(value: unknown): WorkspaceSearchPreferences;
/**
 * The fields of preferences that differ from the defaults, as browser storage
 * keeps them: a field equal to its default is left out, so a later build's
 * default reaches it.
 * @param preferences - the preferences to store.
 * @returns the differing fields, or `undefined` when nothing differs from {@link defaultSearchPreferences}.
 */
export declare function changedSearchPreferences(preferences: WorkspaceSearchPreferences): Partial<WorkspaceSearchPreferences> | undefined;
/**
 * Whether the tree treats an entry as hidden: its name starts with `.`. The
 * Host's search judges names the same way, except on a Windows Host, where
 * ripgrep also treats an entry with the hidden file attribute as hidden. The
 * tree lists names one level at a time under the workspace root, so a name is
 * judged on its own.
 * @param entry - a listed entry.
 * @returns `true` for a hidden entry.
 */
export declare function isHiddenEntry(entry: {
    readonly name: string;
}): boolean;
//# sourceMappingURL=search-rules.d.ts.map