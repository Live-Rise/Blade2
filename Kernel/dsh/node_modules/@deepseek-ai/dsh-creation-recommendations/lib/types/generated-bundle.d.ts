/** Validate model-written files before profile installation.
 * @param directory - Host-reserved output directory.
 * @param name - Host-reserved package name.
 * @returns after package identity, insert-only rows, built entries, the built `./client` entry of a package declaring
 * `dsh.client`, and portable links have been checked; does not execute code.
 */
export declare function validateGeneratedBundle(directory: string, name: string): Promise<void>;
//# sourceMappingURL=generated-bundle.d.ts.map