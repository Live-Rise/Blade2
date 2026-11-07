import type { Check } from "./types.ts";
/** What a schema evaluated: property names (or all of them), the evaluated item prefix, items matched by `contains`. */
export interface Annotations {
    props: Set<string> | null;
    allProps: boolean;
    items: number;
    allItems: boolean;
    indexes: Set<number> | null;
}
/** A check that records what it evaluated into `a` when it passes. */
export type AnnotatingCheck = (value: unknown, a: Annotations) => boolean;
/** Empty annotations. */
export declare function newAnnotations(): Annotations;
export declare const ANNOTATING_TRUE: AnnotatingCheck;
export declare const ANNOTATING_FALSE: AnnotatingCheck;
/** The indirection a recursive reference goes through. */
export declare function indirectAnnotating(slot: {
    check: AnnotatingCheck | undefined;
}): AnnotatingCheck;
/** A plain check in annotating position: it evaluates nothing that unevaluated* sees. */
export declare function ignoreAnnotations(check: Check): AnnotatingCheck;
/** An annotating check run as a plain check, with fresh annotations. */
export declare function withFreshAnnotations(check: AnnotatingCheck): Check;
/** `check` with `uri` pushed onto the dynamic scope while it runs. */
export declare function scopedAnnotating(scope: string[], uri: string, check: AnnotatingCheck): AnnotatingCheck;
/** unevaluated* sees only the annotations of its own schema, never those of its siblings in the parent. */
export declare function isolateAnnotations(check: AnnotatingCheck): AnnotatingCheck;
/** The non-annotating keywords, then each annotating part in order. */
export declare function annotateAll(plain: Check, parts: readonly AnnotatingCheck[]): AnnotatingCheck;
/** `properties`, `patternProperties` and `additionalProperties`. */
export declare function annotateObject(props: readonly (readonly [string, Check])[], patterns: readonly (readonly [RegExp, Check])[], additional: Check | undefined): AnnotatingCheck;
/** `prefixItems`/`items`/`additionalItems` and `contains` with its bounds. */
export declare function annotateArray(prefix: readonly Check[], rest: Check | undefined, contains: Check | undefined, minContains: number, maxContains: number): AnnotatingCheck;
/** `anyOf` (or `oneOf` when `exactlyOne`): every passing branch contributes its annotations. */
export declare function annotateAny(branches: readonly AnnotatingCheck[], exactlyOne: boolean): AnnotatingCheck;
/** `if`, `then` and `else`: a passing `if` contributes its annotations even without `then`. */
export declare function annotateIf(cond: AnnotatingCheck, then: AnnotatingCheck | undefined, otherwise: AnnotatingCheck | undefined): AnnotatingCheck;
/** One `dependentSchemas` entry. */
export declare function annotateDependent(key: string, check: AnnotatingCheck): AnnotatingCheck;
/** `unevaluatedProperties`: every property no other keyword evaluated passes `check`. */
export declare function unevaluatedProperties(check: Check): AnnotatingCheck;
/** `unevaluatedItems`: every item no other keyword evaluated passes `check`. */
export declare function unevaluatedItems(check: Check): AnnotatingCheck;
//# sourceMappingURL=annotations.d.ts.map