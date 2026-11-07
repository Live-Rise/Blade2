import { type Annotations, type Properties, type Schema, type Static, type StaticObject } from "@earendil-works/pi-schema";
/** An object that allows other properties; `additionalProperties: true` says so explicitly. */
export declare function openObject<const P extends Properties>(properties: P, options?: Annotations<StaticObject<P>> & {
    readonly additionalProperties?: true;
}): Schema<StaticObject<P>>;
/** One string: `{ type: "string", const: value }`. */
export declare function stringLiteral<const V extends string>(value: V, options?: Annotations<V>): Schema<V>;
/** An object with any string keys and values of `value`: `{ type: "object", patternProperties: { "^.*$": value } }`. */
export declare function stringRecord<S extends Schema>(value: S, options?: Annotations<Record<string, Static<S>>>): Schema<Record<string, Static<S>>>;
//# sourceMappingURL=schema-builders.d.ts.map