/** Run package management for a profile.
 * @param profile Profile name.
 * @param args DSH exemption command or pnpm arguments relative to the invoking directory.
 * @returns Zero on success; nonzero on invalid approval or package-manager failure.
 */
export declare function runPlugin(profile: string, args: readonly string[]): Promise<number>;
//# sourceMappingURL=plugin.d.ts.map