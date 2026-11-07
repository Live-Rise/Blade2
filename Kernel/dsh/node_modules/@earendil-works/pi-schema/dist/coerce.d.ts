import type { Compiler } from "./compiler.ts";
/** Coerces values for one compiled schema. */
export declare class Coercer {
    private readonly c;
    private readonly schema;
    /**
     * The (schema node, value) pairs pass 2 is converting through a re-entry point, as a stack. Conversion can enter
     * a node again with the same value only through a `$ref` or by wrapping a single value into an array (also for
     * schemas that cycle through shared JS objects): a recursive schema that meets a value no branch accepts would
     * convert it without end. A pair already on the stack (primitives by value, objects by reference) leaves the value
     * unchanged. Only those two points push, so schemas without references or wraps pay nothing.
     */
    private readonly entered;
    /** How often a re-entry was cut short in this coerce() call; results computed across one are not memoized. */
    private cuts;
    /**
     * Results of `$ref` targets by (node, value) within one coerce() call: a recursive union whose branches all
     * convert the same children (and all fail) would convert them once per branch, exponentially in the depth.
     */
    private memo;
    constructor(compiler: Compiler, schema: unknown);
    /** `value` coerced: a copy along the changed paths only, or `value` itself when nothing changes. */
    coerce(value: unknown): unknown;
    /** Pass 2 alone, as TypeBox's `Value.Convert` (the TypeBox conformance tests run it). */
    convertOnly(value: unknown): unknown;
    /** `node` converted for `value` at a re-entry point: unchanged when that pair is already being converted. */
    private reenter;
    /** Whether `value` passes the schema `node`; undefined when `node` cannot be compiled. */
    private passes;
    private baseOf;
    private dropOptionalNulls;
    private convert;
    /** One schema node's conversion: its first keyword among `const`, `enum`, the unions, `allOf` and `type`. */
    private convertNode;
    /** An enum is a union of its values: a member stays, else the first value the input converts to. */
    private convertEnum;
    /**
     * A value one branch accepts stays; else the first branch's conversion that the whole union accepts. Branches are
     * converted in order and only until one is accepted, so a later recursive branch is not entered needlessly. A node
     * that also declares members (plain JSON Schema; TypeBox's never do) converts them into each candidate before it is
     * checked, so a branch is not discarded for a sibling property that is converted after it.
     */
    private convertUnion;
    /** The object members of a union or intersection node, when it declares any. */
    private convertMembers;
    private convertArray;
    private convertObject;
    private coerceTypes;
    private coerceUnion;
    private coerceProperties;
    private coerceItems;
}
//# sourceMappingURL=coerce.d.ts.map