/**
 * Record the build scripts a failed run reported as blocked, then list every `allowBuilds` key awaiting a decision.
 *
 * pnpm writes those policy entries itself only when it can prompt, and the
 * manager always runs it non-interactively, so the run's own report is the
 * record that survives.
 * @param dir Current profile directory.
 * @param output Captured output of the failed run.
 * @returns Exact `allowBuilds` keys awaiting a build decision; wildcard rules are excluded.
 */
export declare function recordPendingBuilds(dir: string, output: string): Promise<string[]>;
/**
 * Read the `allowBuilds` keys a pnpm run reported as ignored build scripts or as a git-hosted package it refused to prepare.
 * @param output Captured output of the run.
 * @returns Exact keys in report order; wildcard patterns and git keys carrying a credential are excluded.
 */
export declare function blockedBuildNames(output: string): string[];
/** Persist approval without running scripts; the caller holds the profile manifest lock.
 * @param dir Current profile directory.
 * @param names Explicit package names from the pending build list.
 * @throws If a name is no longer pending or allowBuilds contains YAML anchors or aliases; no approvals are written.
 */
export declare function approveBuilds(dir: string, names: readonly string[]): Promise<void>;
//# sourceMappingURL=build-approval.d.ts.map