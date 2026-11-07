/**
 * Unpadded base64url encoding for identifiers and credentials.
 *
 * Some execution targets run a Buffer polyfill without the `base64url` encoding
 * name. Encoding through standard base64 keeps those targets working while the
 * output stays the RFC 4648 base64url form that Relay records use.
 * @param value - Bytes to encode.
 * @returns The unpadded base64url text.
 */
export declare function base64Url(value: Uint8Array): string;
/**
 * Decode unpadded base64url text.
 * @param text - RFC 4648 base64url text without padding.
 * @returns The decoded bytes.
 */
export declare function fromBase64Url(text: string): Buffer;
//# sourceMappingURL=base64-url.d.ts.map