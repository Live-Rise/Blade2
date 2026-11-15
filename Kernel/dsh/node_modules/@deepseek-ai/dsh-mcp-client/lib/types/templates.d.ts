/**
 * `${credential:REF}` syntax and substitution for MCP transport strings.
 * Managers validate syntax at configuration writes; the client resolves values
 * at runtime without changing the stored configuration.
 *
 * @module
 */
import { McpLocalError } from './diagnostic.ts';
/** A referenced credential has no configured value. */
export declare class MissingTemplateValue extends McpLocalError {
}
/**
 * The credential references one configured string names, read statically. A
 * rotation observer uses this over the raw configuration so a server whose
 * earlier resolution failed — a credential configured only now — is still
 * recognized as affected.
 * @param value - the configured string.
 * @returns the referenced credential names, in template order.
 */
export declare function credentialRefsIn(value: string): string[];
/**
 * Reject a string whose `${…}` occurrences are not all well-formed template
 * tokens, naming the offending occurrence and where it sits.
 * @param value - the configured string.
 * @param where - configuration path used in the thrown message.
 */
export declare function assertTemplateSyntax(value: string, where: string): void;
/**
 * Resolve every template token in one string.
 * @param value - the configured string.
 * @param where - configuration path used in thrown messages.
 * @param credential - credential-reference lookup returning `undefined` while unconfigured.
 * @returns the substituted string.
 * @throws when a referenced credential is unconfigured.
 */
export declare function resolveTemplateString(value: string, where: string, credential: (ref: string) => Promise<string | undefined>): Promise<string>;
//# sourceMappingURL=templates.d.ts.map