const BUILTINS = {
    goal: '@deepseek-ai/dsh-command-goal',
    plan: '@deepseek-ai/dsh-plan-mode',
    feedback: '@deepseek-ai/dsh-command-feedback',
    compact: '@deepseek-ai/dsh-command-compact',
    permission: '@deepseek-ai/dsh-permission-presets',
    export: '@deepseek-ai/dsh-session-log-export',
};
/** English and Simplified Chinese spellings recognized by command input. */
export const builtinCommandTokens = {
    en: { goal: 'goal', plan: 'plan', feedback: 'feedback', compact: 'compact', permission: 'permission', export: 'export' },
    zh: { goal: '目标', plan: '计划', feedback: '反馈', compact: '压缩', permission: '权限', export: '导出' },
};
/**
 * Identify a first-party definition after scoped shadowing.
 * @param descriptor - effective descriptor from the receiving Agent's catalog.
 * @returns its capability name, or undefined for another definition.
 */
export function builtinCommandName(descriptor) {
    return Object.keys(BUILTINS)
        .find(name => descriptor.definitionId === BUILTINS[name]);
}
const TOKEN_ALIASES = new Map(Object.keys(BUILTINS).flatMap(name => [builtinCommandTokens.zh[name], builtinCommandTokens.en[name]].map(token => [token, name])));
/**
 * Resolve typed spelling without overriding an exact scoped command name.
 * @param token - typed name without its leading slash.
 * @param descriptors - effective descriptors in the receiving Agent's catalog.
 * @returns the matching descriptor; aliases only select their first-party definition.
 */
export function resolveCommand(token, descriptors) {
    const exact = descriptors.find(descriptor => descriptor.name === token);
    if (exact !== undefined)
        return exact;
    const name = TOKEN_ALIASES.get(token);
    if (name === undefined)
        return undefined;
    return descriptors.find(descriptor => descriptor.definitionId === BUILTINS[name]);
}
//# sourceMappingURL=builtin.js.map