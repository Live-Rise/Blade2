import type { InputHTMLAttributes, ReactNode } from 'react';
import type { en } from './locales.ts';
/** Input attributes and section copy for the current credential draft. */
interface ApiKeyInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'type' | 'className' | 'autoComplete'> {
    /** Unsaved input; stored credentials are never read into this field. */
    readonly value: string;
    /** Server-masked preview and actions; the full stored credential stays on the Host. */
    readonly saved?: {
        readonly masked: string;
        readonly clear?: () => void;
    };
    /** Section copy, including the same reveal labels as SecretText. */
    readonly t: (key: keyof typeof en) => string;
}
/**
 * Reveal only a nonempty draft. Saved keys allow whole replacement or deletion.
 * @param props - current draft, input attributes, and translated control labels.
 * @returns a password input with an optional reveal button.
 */
export declare function ApiKeyInput({ value, saved, t, ...props }: ApiKeyInputProps): ReactNode;
export {};
//# sourceMappingURL=ApiKeyInput.d.ts.map