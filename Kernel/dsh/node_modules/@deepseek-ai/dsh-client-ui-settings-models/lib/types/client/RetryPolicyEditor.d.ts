/**
 * The provider retry-policy fields of an editor card, built from the
 * `retryPolicy` schema the owning namespace publishes: its union branches are
 * the modes, each branch's fields are the inputs, and the field bounds and
 * defaults come from the schema. Number fields render as inputs, string lists
 * as a code picker, and nested objects as a fold. An absent `retryPolicy`
 * keeps the inherited policy; choosing a mode writes a user-layer policy whose
 * empty fields keep the inherited values.
 */
import type { ReactNode } from 'react';
import type { SettingsSchemaOperations } from './schema-operations.ts';
import type { ModelsKey } from './locales.ts';
/** One schema node of the provider's settings form. */
type SchemaNode = NonNullable<ReturnType<SettingsSchemaOperations['nodeAtPath']>>;
/** The inherited policy beneath the user layer, and the schema branch of each mode. */
export interface RetryPolicyDefaults {
    /** Modes in schema order. */
    modes: readonly string[];
    /** Mode in effect when the user layer names none. */
    mode: string;
    /** Schema branch of a stored `mode` value, or undefined when no branch declares it. */
    branch: (mode: unknown) => SchemaNode | undefined;
    /** Inherited value of one field of a branch: the composition layer, then the branch default. */
    value: (branch: SchemaNode, path: readonly string[]) => unknown;
}
/**
 * Resolve the modes and inherited policy beneath the user layer.
 * @param schema - settings schema operations.
 * @param node - the `retryPolicy` schema union.
 * @param inherited - the policy the composition layer sets, if any.
 * @returns the modes and the inherited mode and field values.
 */
export declare function retryPolicyDefaults(schema: SettingsSchemaOperations, node: SchemaNode, inherited: unknown): RetryPolicyDefaults;
/** A refused drafted policy: the message and, when one field causes it, that field. */
export interface RetryPolicyFailure {
    message: string;
    path?: readonly string[];
}
/**
 * The first refused field of a drafted policy, checked against the selected
 * branch's field bounds with empty fields inherited, then against the
 * cross-field rules `resolveRetryPolicy` enforces beyond the schema, then
 * against the whole schema.
 * @param schema - settings schema operations.
 * @param node - the `retryPolicy` schema union.
 * @param policy - the drafted user-layer policy; undefined inherits.
 * @param defaults - the inherited policy beneath the draft.
 * @param t - section copy.
 * @returns the failure, or undefined when the policy is valid.
 */
export declare function retryPolicyFailure(schema: SettingsSchemaOperations, node: SchemaNode, policy: unknown, defaults: RetryPolicyDefaults, t: (key: ModelsKey) => string): RetryPolicyFailure | undefined;
/** Props of {@link RetryPolicyEditor}. */
export interface RetryPolicyEditorProps {
    /** Drafted user-layer policy; undefined inherits. */
    policy: unknown;
    /** Inherited policy and schema branches beneath the draft. */
    defaults: RetryPolicyDefaults;
    /** Failure from {@link retryPolicyFailure}. */
    failure: RetryPolicyFailure | undefined;
    /** Id of the element showing the failure message. */
    failureId: string;
    /** Show the failure message below the fields; otherwise the owner shows it. */
    showFailure: boolean;
    /** Section copy. */
    t: (key: ModelsKey) => string;
    /** Disable edits. */
    disabled: boolean;
    /** Replace the drafted policy; undefined removes the user-layer policy. */
    onChange: (next: Record<string, unknown> | undefined) => void;
}
/**
 * Render the retry mode and its policy fields.
 * @param props - drafted policy, inherited values, and the draft writer.
 * @returns the retry-policy field group.
 */
export declare function RetryPolicyEditor(props: RetryPolicyEditorProps): ReactNode;
export {};
//# sourceMappingURL=RetryPolicyEditor.d.ts.map