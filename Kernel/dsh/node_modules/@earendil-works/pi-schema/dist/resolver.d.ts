import type { CompileOptions, JsonSchema, SchemaObject } from "./types.ts";
/** A schema and the base URI its references resolve against. */
export interface Resolved {
    readonly node: unknown;
    readonly base: string;
}
/** The base URI of a root schema without `$id`; relative references resolve against it. */
export declare const DEFAULT_BASE = "https://json-schema.invalid/root.json";
/**
 * Resolves references in the root schema and the registered documents. The schema is indexed on the first
 * reference or the first `$id` a walker meets, so a schema with neither is never walked for identifiers.
 */
export declare class Resolver {
    readonly draft: "2020-12" | "7";
    /** The base URI the root schema is walked from; walkers apply the root's own `$id` through `baseFor`. */
    readonly rootBase = "https://json-schema.invalid/root.json";
    /** Schema object to the base URI it was indexed under. */
    readonly baseOf: Map<object, string>;
    /** URI (with a `#name` fragment) of every `$dynamicAnchor`, to its schema. */
    readonly dynamicAnchors: Map<string, SchemaObject>;
    private readonly root;
    private readonly options;
    private readonly registry;
    private readonly rejectUnresolved;
    /** The references that named nothing and resolved to `false` (with `unresolvedRefs: "reject"`). */
    private readonly unresolved;
    private readonly resources;
    private readonly anchors;
    private indexed;
    constructor(root: JsonSchema, options: CompileOptions);
    /**
     * The base URI of `node`, whose parent has `base`: its `$id` changes it, unless draft 7 makes the `$id` inert. Once
     * the schema is indexed, the indexed base (a resolved reference target already has its own).
     */
    baseFor(node: SchemaObject, base: string): string;
    /** Indexes the root schema and the registered documents' URIs (once). */
    ensureIndexed(): void;
    /**
     * The schema a reference names, with the base URI it compiles under. Throws if it names nothing, or, with
     * `unresolvedRefs: "reject"`, resolves to `false`, the schema no value passes.
     */
    resolve(ref: string, base: string): Resolved;
    /** Whether `ref`, from `base`, named nothing and resolved to `false`. */
    isUnresolved(ref: string, base: string): boolean;
    private find;
    /** Whether `node` is the root of the schema resource at `base`. */
    isResource(node: object, base: string): boolean;
    private index;
    private load;
    private pointer;
}
/** Whether `value` is a schema object (not `true`/`false`, an array or a primitive). */
export declare function isSchemaObject(value: unknown): value is SchemaObject;
/** `ref` resolved against `base`. URNs and other bases a URL parser rejects take fragment-only references. */
export declare function resolveUri(ref: string, base: string): string;
/** `uri` without its fragment. */
export declare function documentOf(uri: string): string;
//# sourceMappingURL=resolver.d.ts.map