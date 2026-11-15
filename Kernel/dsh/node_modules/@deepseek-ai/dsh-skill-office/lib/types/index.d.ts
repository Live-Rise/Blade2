/** Bundled Office workflows and filesystem resources for document authoring and operations. */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
/**
 * Python source for Office scripts: `'workspace-dependencies'` directs the model to the bundled interpreter and libraries
 * returned by `load_workspace_dependencies`; `'user'` directs it to an existing project or user Python.
 */
export type PythonEnvironment = 'workspace-dependencies' | 'user';
/** Office skill resources and Python environment. */
export interface Config {
    /** Absolute assets directory containing skill folders; defaults to packaged assets. */
    assetRoot?: string;
    /** Python environment named in the Python authoring and verification skills. @default 'workspace-dependencies' */
    pythonEnvironment?: PythonEnvironment;
}
/** Configuration after schema defaults apply. */
export interface ResolvedConfig extends Config {
    /** Python environment named in the Python authoring and verification skills. */
    pythonEnvironment: PythonEnvironment;
}
/** Validated resource configuration. */
export declare const Config: z<Config, ResolvedConfig>;
/** Cordis plugin identity. */
export declare const name = "skill-office";
/** Registry used by the bundled provider. */
export declare const inject: string[];
/**
 * Register Office skills whose `SKILL.md` files the Host reads from `assetRoot`.
 * Skill files are read during catalog discovery and loading, and each distinct failure is reported once. A missing
 * or invalid skill is omitted from a complete catalog, and its failed load returns `undefined` and invalidates the
 * catalog. Any other read error leaves discovery incomplete, so the registry retries it, and rejects a load.
 * @param ctx - Context carrying the skill registry.
 * @param config - Resolved configuration; `assetRoot` falls back to the packaged assets.
 * @throws When `assetRoot` is not absolute.
 */
export declare function apply(ctx: Context, config: ResolvedConfig): void;
//# sourceMappingURL=index.d.ts.map