/**
 * Model-facing agent names. Each child's name is unique among its creator's
 * children and is recorded in the creator's catalog; a member's full name is
 * the slash-separated path of names from the root, whose own name is `lead`.
 *
 * @module @deepseek-ai/dsh-subagent/names
 */
import type { Agent } from '@deepseek-ai/dsh-agent';
/** The root agent's fixed name; no child may take it. */
export declare const ROOT_AGENT_NAME = "lead";
/**
 * Whether a string is a valid child name.
 * @param value - candidate name segment.
 * @returns true for 1–32 lower-case letters, digits, or hyphens starting with a letter, other than `lead`.
 */
export declare function isAgentName(value: string): boolean;
/**
 * Append a child's name to its creator's path. The root's path is the empty string.
 * @param parentPath - the creator's path, or undefined when an ancestor below the root has no name.
 * @param name - the child's catalog name, absent on historical entries.
 * @returns the child's path, or undefined when any link is unnamed.
 */
export declare function joinAgentPath(parentPath: string | undefined, name: string | undefined): string | undefined;
/**
 * Select the model-facing name of one tree member.
 * @param path - the member's path; `''` for the root and undefined for unnamed history.
 * @param id - the member's Session id, used when no complete path exists.
 * @returns `lead`, the full path, or the Session id.
 */
export declare function agentAddress(path: string | undefined, id: string): string;
/**
 * Remove the last name from a descendant path.
 * @param path - a non-empty descendant path.
 * @returns the creator's path; `''` when the creator is the root.
 */
export declare function parentAgentPath(path: string): string;
/** A sibling name held from admission until the creator's catalog commits or startup fails. */
export interface SiblingNameReservation {
    readonly name: string;
    /** Return the name to the pool; repeated calls are harmless. */
    release(): void;
}
/** Names claimed by in-flight creations, keyed by the exact live creator. */
export declare class SiblingNameReservations {
    private readonly pending;
    /**
     * Claim a name synchronously before startup awaits, so concurrent siblings cannot share it.
     * @param parent - exact live creator whose catalog receives the name.
     * @param committed - names already recorded in the creator's catalog.
     * @param requested - requested name; omission selects the first free `agent-N`.
     * @returns the claimed name and its release.
     * @throws {@link SubagentError} `INVALID_AGENT_NAME` or `DUPLICATE_AGENT_NAME`.
     */
    reserve(parent: Agent, committed: ReadonlySet<string>, requested?: string): SiblingNameReservation;
}
//# sourceMappingURL=names.d.ts.map