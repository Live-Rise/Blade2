/** Pure configuration changes for Node PTC profile editors. */
import type { PtcProgramFormat } from '@deepseek-ai/dsh-ptc-runtime';
import type { Config } from './config.ts';
/**
 * Select retained Node execution while preserving Script syntax.
 * @param programFormat - Effective syntax; omission uses the Node provider's default function-body syntax.
 * @returns Both execution settings to merge into the profile atomically; function bodies switch to the retained REPL.
 */
export declare function resolvePersistentExecution(programFormat?: PtcProgramFormat): Pick<Required<Config>, 'executionMode' | 'scriptPersistence'>;
//# sourceMappingURL=configuration.d.ts.map