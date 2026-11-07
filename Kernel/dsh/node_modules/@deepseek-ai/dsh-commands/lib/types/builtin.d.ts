/** First-party command identities and input spellings shared by Host and Client adapters. */
import type { CommandDescriptor } from './types.ts';
declare const BUILTINS: {
    readonly goal: "@deepseek-ai/dsh-command-goal";
    readonly plan: "@deepseek-ai/dsh-plan-mode";
    readonly feedback: "@deepseek-ai/dsh-command-feedback";
    readonly compact: "@deepseek-ai/dsh-command-compact";
    readonly permission: "@deepseek-ai/dsh-permission-presets";
    readonly export: "@deepseek-ai/dsh-session-log-export";
};
/** Stable first-party capabilities, independent of their registered command names. */
export type BuiltinCommandName = keyof typeof BUILTINS;
/** English and Simplified Chinese spellings recognized by command input. */
export declare const builtinCommandTokens: {
    readonly en: {
        readonly goal: "goal";
        readonly plan: "plan";
        readonly feedback: "feedback";
        readonly compact: "compact";
        readonly permission: "permission";
        readonly export: "export";
    };
    readonly zh: {
        readonly goal: "目标";
        readonly plan: "计划";
        readonly feedback: "反馈";
        readonly compact: "压缩";
        readonly permission: "权限";
        readonly export: "导出";
    };
};
/**
 * Identify a first-party definition after scoped shadowing.
 * @param descriptor - effective descriptor from the receiving Agent's catalog.
 * @returns its capability name, or undefined for another definition.
 */
export declare function builtinCommandName(descriptor: CommandDescriptor): BuiltinCommandName | undefined;
/**
 * Resolve typed spelling without overriding an exact scoped command name.
 * @param token - typed name without its leading slash.
 * @param descriptors - effective descriptors in the receiving Agent's catalog.
 * @returns the matching descriptor; aliases only select their first-party definition.
 */
export declare function resolveCommand(token: string, descriptors: readonly CommandDescriptor[]): CommandDescriptor | undefined;
export {};
//# sourceMappingURL=builtin.d.ts.map