/**
 * Model-facing Office tools render document pages, convert documents, and recalculate workbooks through the shared
 * `officeKit` service. Tools are admitted after an Office skill loads; a missing service makes admitted calls fail
 * as `unavailable`. Reads use the filesystem backend's read authority; each call reads
 * one source byte before the service runs, so a cached result never answers a call that cannot read the source.
 * Converted files stay in the Office kit's private output store; the model copies them into the workspace with the
 * shell, so these tools write no workspace file.
 * @module @deepseek-ai/dsh-tool-office
 */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
/** Cordis plugin name used by Loader diagnostics. */
export declare const name = "tool-office";
/** Required services; `officeKit` is read at each call, and `attachments` and `llm` are checked when rendering. */
export declare const inject: string[];
/** Activation skills and render bounds; lower deployment attachment limits take precedence. */
export interface Config {
    /** Skill names whose successful loading admits all three tools from the following step. */
    activationSkills: string[];
    /** Maximum images one render returns, including worksheet fragments. */
    maxImages: number;
    /** Maximum pixels of each image. */
    maxImagePixels: number;
    /** Maximum width and height of each image in pixels. */
    maxImageDimension: number;
    /** Maximum PNG bytes across all images of one render. */
    maxImageBytes: number;
}
/** Validated activation skills and render bounds. */
export declare const Config: z<Partial<Config>, Config>;
/**
 * Register Office tools admitted after a successful skill load or an earlier Office declaration.
 * @param ctx - registration scope; disposing it removes the tools.
 * @param config - validated activation skills and render bounds.
 */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map