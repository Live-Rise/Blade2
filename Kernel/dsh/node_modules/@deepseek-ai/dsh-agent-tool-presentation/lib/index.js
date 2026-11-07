import z from "@deepseek-ai/schemastery";
import * as NativeToolExecutor from "@deepseek-ai/dsh-tool-executor-native";
import * as PtcToolExecutor from "@deepseek-ai/dsh-tool-executor-ptc";
//#region lib/types/index.js
/**
* Select built-in tool execution for every Agent in the mounting scope.
* @module
* @deprecated Mount @deepseek-ai/dsh-tool-executor-native or @deepseek-ai/dsh-tool-executor-ptc directly.
*/
/** Cordis plugin name. */
const name = "tool-presentation";
/**
* Native selection does not require a program runtime; the PTC child waits
* for its runtime before registering an executor in this scope.
*/
const inject = ["tools"];
/**
* Runtime schema.
* @deprecated Select the native or PTC executor plugin directly instead of configuring a mode.
*/
const Config = z.object({ mode: z.union(["native", "ptc"]).required() });
/**
* Declare the tool presentation for every agent this composition covers.
* @param ctx - the mounting composition's scope context (a preset's standing scope).
* @param config - the selected presentation.
* @deprecated Mount @deepseek-ai/dsh-tool-executor-native or @deepseek-ai/dsh-tool-executor-ptc directly.
*/
function apply(ctx, config) {
	ctx.plugin(config.mode === "native" ? NativeToolExecutor : PtcToolExecutor);
}
//#endregion
export { Config, apply, inject, name };
