import type { Context } from '@deepseek-ai/cordis';
import type { ReadFamilyRowProps } from './read-family-row.tsx';
/** Office file row props: the runtime share, file actions, and locale seat. */
export type OfficeRowProps = ReadFamilyRowProps;
/**
 * Show an Office call with input-path navigation and its result text.
 * @param props - tool call, file actions, and locale seat.
 * @returns the Office result row.
 */
export declare function OfficeRow({ toolName, block, cwd, home, openFile, resolveFilePath, sessionId, inspect, useDisclosure, useContextMenu, t, }: OfficeRowProps): import("react").JSX.Element;
/** Registers the three Office toolviews. */
export declare const officeToolview: {
    name: string;
    inject: string[];
    /**
     * Register Office rows into the Tool-owned keyed view slot.
     * @param ctx - registrant context (disposal rides ctx.effect inside slots.register).
     */
    apply(ctx: Context): void;
};
//# sourceMappingURL=office-row.d.ts.map