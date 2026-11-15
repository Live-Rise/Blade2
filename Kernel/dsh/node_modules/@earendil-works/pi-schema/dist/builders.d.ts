import type { TypeBoxStatic } from "./typebox-static.ts";
declare const STATIC: unique symbol;
declare const OPTIONAL_TYPE: unique symbol;
/** A JSON Schema object whose valid values have type `T` (a type-level brand; nothing extra exists at run time). */
export type Schema<T = unknown> = {
    readonly [keyword: string]: unknown;
} & {
    readonly [STATIC]?: T;
};
/**
 * The type of the values `S` accepts: the type a builder recorded; for a TypeBox schema (`'~kind'` or `'~unsafe'`),
 * TypeBox's static type for the common kinds (see `TypeBoxStatic`); else `unknown`.
 */
export type Static<S> = S extends {
    readonly "~kind": unknown;
} | {
    readonly "~unsafe": unknown;
} ? TypeBoxStatic<S> : S extends {
    readonly [STATIC]?: infer T;
} ? unknown extends T ? unknown : T : unknown;
/** Annotations every builder accepts: they describe the schema and never change what it accepts. */
export interface Annotations<T = unknown> {
    readonly title?: string;
    readonly description?: string;
    readonly examples?: readonly T[];
    readonly deprecated?: boolean;
    /** A default value, an annotation for tools and forms; validation never inserts it. */
    readonly default?: T;
}
/** Length bounds of a string, an array or bytes. */
export interface LengthBounds {
    readonly minLength?: number;
    readonly maxLength?: number;
}
/** A string, with optional length bounds (in code points), pattern and format. */
export declare function str(options?: Annotations<string> & LengthBounds & {
    readonly pattern?: string;
    readonly format?: string;
}): Schema<string>;
/** Bounds of a number. */
export interface NumberBounds {
    readonly minimum?: number;
    readonly maximum?: number;
    readonly exclusiveMinimum?: number;
    readonly exclusiveMaximum?: number;
    readonly multipleOf?: number;
}
/** A number within optional bounds. */
export declare function num(options?: Annotations<number> & NumberBounds): Schema<number>;
/** An integer within optional bounds. */
export declare function int(options?: Annotations<number> & NumberBounds): Schema<number>;
/** Bounds of a bigint: TypeBox's `Type.BigInt` options. */
export interface BigintBounds {
    readonly minimum?: bigint;
    readonly maximum?: bigint;
    readonly exclusiveMinimum?: bigint;
    readonly exclusiveMaximum?: bigint;
    readonly multipleOf?: bigint;
}
/**
 * A bigint within optional bounds (`{ type: "bigint" }`, as TypeBox's `Type.BigInt`): an extension of JSON Schema for
 * values that never pass through JSON, such as 64-bit integers a Parquet reader decodes.
 */
export declare function bigint(options?: Annotations<bigint> & BigintBounds): Schema<bigint>;
/** A boolean. */
export declare function bool(options?: Annotations<boolean>): Schema<boolean>;
/** `null`. */
export declare function nul(options?: Annotations<null>): Schema<null>;
/** Any value. */
export declare function unknown(options?: Annotations): Schema<unknown>;
/** Exactly `value` (a string, number, boolean or null). */
export declare function lit<const V extends string | number | boolean | null>(value: V, options?: Annotations<V>): Schema<V>;
/** One of the given values. */
export declare function oneOf<const V extends readonly (string | number | boolean | null)[]>(values: V, options?: Annotations<V[number]>): Schema<V[number]>;
/**
 * Any of the given schemas. A union of objects tagged by a `lit()` property (`{ kind: lit("text"), ... }`) is checked
 * by the tag: only the branch it selects runs, and its errors are the ones reported.
 */
export declare function union<const S extends readonly Schema[]>(schemas: S, options?: Annotations<Static<S[number]>>): Schema<Static<S[number]>>;
/** `schema` or `null`. */
export declare function nullable<T>(schema: Schema<T>, options?: Annotations<T | null>): Schema<T | null>;
/** An array of `items`, with optional length bounds and unique items. */
export declare function arr<T>(items: Schema<T>, options?: Annotations<T[]> & {
    readonly minItems?: number;
    readonly maxItems?: number;
    readonly uniqueItems?: boolean;
}): Schema<T[]>;
/** A fixed-length array whose items match `items` in order. */
export declare function tuple<const S extends readonly Schema[]>(items: S, options?: Annotations<{
    -readonly [K in keyof S]: Static<S[K]>;
}>): Schema<{
    -readonly [K in keyof S]: Static<S[K]>;
}>;
/** An object with string keys and values of `value` (a record). */
export declare function map<T>(value: Schema<T>, options?: Annotations<Record<string, T>>): Schema<Record<string, T>>;
/**
 * `schema` typed as accepting `T`, for values the builders cannot express (a type checked elsewhere, a schema from
 * another system). The schema is returned as it is: nothing checks that its values are `T`.
 */
export declare function unsafe<T>(schema?: Schema): Schema<T>;
/**
 * A schema that contains itself, as a tree's children are trees: `build` receives a reference to the schema it
 * returns. TypeScript cannot infer a recursive type from the builders, so `T` is given:
 * `recursive<Tree>("Tree", (tree) => obj({ name: str(), children: arr(tree) }))`. `id` becomes the schema's `$id`
 * and the target of the reference, so it must be unique among the recursive schemas within one schema.
 *
 * The counterpart of TypeBox's `Type.Cyclic` + `Type.Ref`, with other JSON: an `$id` on the schema instead of `$defs`
 * and a root `$ref`. It validates the same values. The JSON differs only where a schema is published, and its type is
 * written once instead of inferred, which a recursive schema (a file tree) needs only once.
 */
export declare function recursive<T>(id: string, build: (self: Schema<T>) => Schema<T>): Schema<T>;
/** A Uint8Array with optional length bounds (an extension of JSON Schema for values that never pass through JSON). */
export declare function bytes(options?: Annotations<Uint8Array> & LengthBounds): Schema<Uint8Array>;
/** An optional property of obj(): `{ name: opt(str()) }` may omit `name` or set it to undefined. */
export declare class Optional<S extends Schema = Schema> {
    readonly [OPTIONAL_TYPE]?: S;
    readonly schema: S;
    constructor(schema: S);
}
/** Marks an obj() property as optional. */
export declare function opt<S extends Schema>(schema: S): Optional<S>;
/** The properties of obj(): a schema or an opt() schema per name. */
export type Properties = {
    readonly [name: string]: Schema | Optional;
};
/** The value type of an obj() schema. */
export type StaticObject<P extends Properties> = Simplify<{
    -readonly [K in keyof P as P[K] extends Optional ? never : K]: Static<P[K]>;
} & {
    -readonly [K in keyof P as P[K] extends Optional ? K : never]?: P[K] extends Optional<infer S> ? Static<S> : never;
}>;
type Simplify<T> = {
    [K in keyof T]: T[K];
} & {};
/**
 * A closed object: the given properties (required unless opt()) and no others. `open: true` allows other
 * properties, as tool schemas from other systems often do.
 */
export declare function obj<const P extends Properties>(properties: P, options?: Annotations<StaticObject<P>> & {
    readonly open?: boolean;
}): Schema<StaticObject<P>>;
export {};
//# sourceMappingURL=builders.d.ts.map