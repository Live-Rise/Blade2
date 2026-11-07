import type { Check } from "./types.ts";
export declare const ALWAYS: Check;
export declare const NEVER: Check;
export declare const IS_ARRAY: Check;
export declare const IS_OBJECT: Check;
/** Whether `v` is a JSON object: not null, an array or bytes. */
export declare function isObject(v: unknown): v is Record<string, unknown>;
/** The check of a `type` name; `bytes` (a Uint8Array) extends JSON Schema for values that never pass through JSON. */
export declare const TYPE_CHECKS: ReadonlyMap<string, Check>;
/** Leaf checks that object and array loops run inline by code instead of calling them. */
export declare const LEAF_CODES: ReadonlyMap<Check, number>;
/** Whether a combined object or array check also checks the type: not at all, strictly, or passing other types. */
export declare const GUARD_NONE = 0;
export declare const GUARD_STRICT = 1;
export declare const GUARD_LOOSE = 2;
export type Guard = typeof GUARD_NONE | typeof GUARD_STRICT | typeof GUARD_LOOSE;
/** Flags of a declared property in an object check's entries: its leaf code and two bits. */
export declare const CODE = 7;
export declare const REQUIRED = 8;
export declare const INHERITED = 16;
/** All of `parts`; two and three parts get closures without a loop. */
export declare function and(parts: readonly Check[]): Check;
/** A check whose implementation is set later: the indirection a recursive reference goes through. */
export declare function indirect(slot: {
    check: Check | undefined;
}): Check;
/** `check` with `uri` pushed onto the dynamic scope while it runs (only for schemas with `$dynamicRef`). */
export declare function scopedCheck(scope: string[], uri: string, check: Check): Check;
/** Any of the named types. */
export declare function typeCheck(types: readonly unknown[]): Check;
/** A Uint8Array within the length bounds. */
export declare function bytesCheck(min: number, max: number): Check;
/** The number of Unicode code points of `s`: JSON Schema string lengths count code points, not UTF-16 units. */
export declare function codePoints(s: string): number;
/** The string keywords: length bounds, pattern and format. */
export declare function stringCheck(strict: boolean, min: number, max: number, re: RegExp | undefined, format: ((value: string) => boolean) | undefined): Check;
/** The number keywords. `mode`: 0 passes other types, 1 requires a number, 2 an integer. */
export declare function numberCheck(mode: 0 | 1 | 2, min: number, max: number, xmin: number, xmax: number, multipleOf: number | undefined): Check;
/** Whether `v` is a multiple of `d`, exactly for decimals: 0.0075 / 0.0001 is 74.99999999999999 in floating point. */
export declare function isMultiple(v: number, d: number): boolean;
/** `inner` for objects; other values pass unless `strict`. */
export declare function guardObject(inner: Check, strict: boolean): Check;
/** `inner` for arrays; other values pass unless `strict`. */
export declare function guardArray(inner: Check, strict: boolean): Check;
/**
 * An object with declared properties: `entries` is `[key, check, flags]` per property. The value's own keys are
 * walked with for...in (whose `o[k]` V8 reads from the enum cache, unlike a load by a name from a list), each found
 * among the declared names; `closed` rejects any other key. A property whose value is `undefined` counts as absent.
 */
export declare function declaredObject(entries: readonly unknown[], requiredCount: number, closed: boolean, guard: Guard): Check;
/** An object with declared properties, required keys not declared, pattern properties and additional properties. */
export declare function openObject(entries: readonly unknown[], keys: readonly string[], extraRequired: readonly string[], patterns: readonly (readonly [RegExp, Check])[], additional: Check | undefined, guard: Guard): Check;
/** Every property name passes `check` (`v` is an object). */
export declare function propertyNamesCheck(check: Check): Check;
/** `minProperties` and `maxProperties` (`v` is an object). */
export declare function propertyCountCheck(min: number, max: number): Check;
/** `dependentRequired` (`v` is an object). */
export declare function dependentRequiredCheck(entries: readonly (readonly [string, readonly string[]])[]): Check;
/** `dependentSchemas` (`v` is an object). */
export declare function dependentSchemasCheck(entries: readonly (readonly [string, Check])[]): Check;
/** The common array: guard, length bounds, prefix items and the items after them, in one closure. */
export declare function listCheck(guard: Guard, min: number, max: number, prefix: readonly Check[], rest: Check | undefined): Check;
/** Prefix items and the items after them (`v` is an array). */
export declare function itemsCheck(prefix: readonly Check[], rest: Check | undefined): Check;
/** `minItems` and `maxItems` (`v` is an array). */
export declare function lengthCheck(min: number, max: number): Check;
/** `contains` with its bounds (`v` is an array). */
export declare function containsCheck(check: Check, min: number, max: number): Check;
/** `uniqueItems` (`v` is an array). */
export declare function uniqueCheck(v: unknown): boolean;
/** `enum`: up to 8 primitives by a scan, more by a Set, objects and arrays by deep equality. */
export declare function enumCheck(values: readonly unknown[]): Check;
/** `const`. */
export declare function constCheck(value: unknown): Check;
/** `anyOf`. */
export declare function anyCheck(checks: readonly Check[]): Check;
/** `oneOf`. */
export declare function oneCheck(checks: readonly Check[]): Check;
/** `not`. */
export declare function notCheck(check: Check): Check;
/** `if`, `then` and `else`. */
export declare function ifCheck(cond: Check, then: Check, otherwise: Check): Check;
/** A union of objects tagged by a const on `key`: the tag selects the one branch that can match. */
export declare function taggedCheck(key: string, tags: readonly unknown[], checks: readonly Check[]): Check;
/** JSON equality: same primitives, arrays with equal items in order, objects with the same keys and equal values. */
export declare function deepEqual(a: unknown, b: unknown): boolean;
//# sourceMappingURL=checks.d.ts.map