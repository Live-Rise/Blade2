import type { Static } from "./builders.ts";
import type { Check, CompileOptions, JsonSchema, ParseResult, ValidationError } from "./types.ts";
/**
 * Compiles `schema` into a Validator. Throws if the schema is malformed or a `$ref` names nothing (register other
 * documents with `options.schemas`). The schema is read, never modified. With `coerce: true` the validator also
 * coerces and parses loosely typed input.
 */
export declare function compile<S extends JsonSchema>(schema: S, options: CompileOptions & {
    readonly coerce: true;
}): CoercingValidator<Static<S>>;
export declare function compile<S extends JsonSchema>(schema: S, options?: CompileOptions): Validator<Static<S>>;
/** A compiled schema whose valid values have type `T` (`unknown` unless the schema was built with the builders). */
export declare class Validator<T = unknown> {
    /** Whether `value` is valid. A plain function: it may be passed around without its validator. */
    readonly check: (value: unknown) => value is T;
    protected readonly schema: JsonSchema;
    protected readonly options: CompileOptions;
    private explainer;
    constructor(schema: JsonSchema, options: CompileOptions, check: Check);
    /**
     * Why `value` is invalid: one error per failing keyword, `[]` when it is valid. Built on the first failure and
     * kept, so a validator whose values always pass never pays for it.
     */
    errors(value: unknown): ValidationError[];
    /**
     * `value` itself when it is valid, else its errors: check() and errors() in one result, without coercion.
     *
     * Not TypeBox's `Value.Parse`, which also clones, fills defaults, converts and strips undeclared properties before
     * it checks. This only checks: the strict replacement where `Value.Parse` is used to validate (its other steps
     * change nothing for a valid value of a schema without defaults or extra properties). Use `parse()` with
     * `coerce: true` for conversion.
     */
    validate(value: unknown): ParseResult<T>;
}
/** A validator compiled with `coerce: true`: it also coerces loosely typed input towards the schema. */
export declare class CoercingValidator<T = unknown> extends Validator<T> {
    private coercer;
    /**
     * `value` coerced towards the schema: numeric strings to numbers, `"true"` to `true`, a single value to a one-item
     * array, an optional `null` dropped, the first union branch the value converts to (the README lists every rule).
     * Only the changed paths are copied; `value` comes back itself when nothing changes, and is never modified. Values
     * that cannot be coerced stay as they are. The rules are built on the first call.
     */
    coerce(value: unknown): unknown;
    /** Coerces `value`, then checks it: the coerced value when it is valid, else its errors. */
    parse(value: unknown): ParseResult<T>;
}
//# sourceMappingURL=validator.d.ts.map