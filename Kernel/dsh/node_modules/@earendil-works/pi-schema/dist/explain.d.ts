import type { Compiler } from "./compiler.ts";
import type { ValidationError } from "./types.ts";
/** Explains failures of one compiled schema. */
export declare class Explainer {
    private readonly c;
    private readonly schema;
    /**
     * The errors of `$ref` targets for object values, within one errors() call: a recursive union explains each branch
     * to pick the one to blame, and every branch would explain the same children again, exponentially in the depth.
     */
    private memo;
    constructor(compiler: Compiler, schema: unknown);
    /** The errors of `value`, at least one (`value` failed the check). */
    errors(value: unknown): ValidationError[];
    private ok;
    private walk;
    private walkRef;
    private walkString;
    private walkBytes;
    private walkNumber;
    private walkObject;
    private walkArray;
    private walkUnion;
    /** How many of the keys of `v` the branch `s` (through a `$ref`) declares in `properties`. */
    private declaredKeys;
    private tagOf;
    private walkUnevaluated;
}
//# sourceMappingURL=explain.d.ts.map