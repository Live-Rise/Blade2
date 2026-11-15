/** Entity lists and receipts for agent, job, terminal, and language-server tools. */
import type { ToolDetailsModel } from '../components/ToolDetails.tsx';
import { type DetailTranslate } from './detail-model-shared.ts';
/**
 * Read a successful delegation receipt from persisted metadata or historical text.
 * @param args - Recorded creation arguments, including description and full prompt.
 * @param text - Single recorded text block; absent for other content layouts.
 * @param meta - Tool-owned result metadata; only absent metadata permits legacy text parsing.
 *   Its `name` is the child's model-facing agent name and is absent from receipts recorded before agents had names.
 * @param t - Conversation translator.
 * @returns Started task details, a historical synchronous reply, or null for unsupported data.
 */
export declare function subagentDetails(args: Record<string, unknown>, text: string | undefined, meta: unknown, t: DetailTranslate): ToolDetailsModel | null;
/**
 * Derive entity lists and operation receipts from supported recorded output formats.
 * @param name - Wire tool name, including Team-scoped aliases.
 * @param args - Parsed recorded arguments.
 * @param text - Successful recorded result text.
 * @param json - Parsed whole-result JSON, or undefined for non-JSON text.
 * @param t - Conversation translator.
 * @returns Compact details, or null when the output format is not recognized.
 */
export declare function controlDetails(name: string, args: Record<string, unknown>, text: string, json: unknown, t: DetailTranslate): ToolDetailsModel | null;
//# sourceMappingURL=control-details-model.d.ts.map