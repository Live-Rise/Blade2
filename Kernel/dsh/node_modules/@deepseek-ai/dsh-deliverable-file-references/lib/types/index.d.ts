/** Shared file-delivery guidance for clients that render desktop-style file references. */
import type { Context } from '@deepseek-ai/cordis';
/** Stable Loader identity. */
export declare const name = "deliverable-file-references";
/** Prompt registry shared by the Web and SDK desktop compositions. */
export declare const inject: string[];
/**
 * Register the desktop file-reference guidance without mounting a GUI.
 * @param ctx - context carrying the prompt registry.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map