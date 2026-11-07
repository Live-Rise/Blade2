import { assertNever } from "@deepseek-ai/dsh-util-values";
//#region lib/types/configuration.js
/**
* Select retained Node execution while preserving Script syntax.
* @param programFormat - Effective syntax; omission uses the Node provider's default function-body syntax.
* @returns Both execution settings to merge into the profile atomically; function bodies switch to the retained REPL.
*/
function resolvePersistentExecution(programFormat = "function-body") {
	switch (programFormat) {
		case "script": return {
			executionMode: "vm-script",
			scriptPersistence: true
		};
		case "function-body":
		case "repl": return {
			executionMode: "persistent-in-memory",
			scriptPersistence: false
		};
		/* v8 ignore next -- The runtime supplies the closed, typed program-format union. */
		default: return assertNever(programFormat);
	}
}
//#endregion
export { resolvePersistentExecution };
