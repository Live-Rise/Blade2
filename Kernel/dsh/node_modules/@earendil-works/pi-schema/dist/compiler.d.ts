import { type AnnotatingCheck } from "./annotations.ts";
import { Resolver } from "./resolver.ts";
import type { Check, CompileOptions, FormatCheck, JsonSchema } from "./types.ts";
/** Thrown when a compile without dynamic scope tracking meets a bookended `$dynamicRef`: compile again with it. */
export declare const RESTART: unique symbol;
/** One compile of a schema. Its caches and indexes are garbage once the root check is built. */
export declare class Compiler {
    readonly r: Resolver;
    readonly draft: "2020-12" | "7";
    readonly formats: Readonly<Record<string, FormatCheck>> | undefined;
    /** Whether checks track the dynamic scope (the schema has a bookended `$dynamicRef`). */
    readonly dynamic: boolean;
    private readonly schema;
    private readonly scope;
    private readonly cache;
    private readonly annotatingCache;
    constructor(schema: JsonSchema, options: CompileOptions, dynamic: boolean);
    /** The check of the root schema. */
    root(): Check;
    /** The check of a schema (cached by schema object; a recursive reference goes through an indirection). */
    check(node: unknown, base: string): Check;
    private build;
    /**
     * The module-level check of a schema that is only a `type` (with annotations), `ALWAYS` for one with no assertion
     * at all; undefined for every other schema. These are the most common subschemas and cost nothing to share.
     */
    private bareType;
    /** Every keyword of `node`, not tracking annotations. */
    private plain;
    private inPlace;
    private union;
    private ref;
    private dynamicRef;
    /** The initial target and anchor name of a `$dynamicRef` that resolves dynamically, or undefined for a plain ref. */
    private bookended;
    private dynamicLookup;
    /** anyOf/oneOf whose branches are all objects with a distinct required const on one key: the tag picks the branch. */
    private tagged;
    private stringGroup;
    private numberGroup;
    private objectGroup;
    private objectCore;
    private arrayGroup;
    /** The annotating check of a schema evaluated in place (only below `unevaluatedProperties`/`unevaluatedItems`). */
    annotating(node: unknown, base: string): AnnotatingCheck;
    private buildAnnotating;
    private annotatingParts;
    private annotatingRef;
    private annotatingDynamicRef;
}
/** Keywords that never affect a check. */
export declare const NEUTRAL_KEYWORDS: Set<string>;
//# sourceMappingURL=compiler.d.ts.map