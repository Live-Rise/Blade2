import type { ReactNode, RefObject } from 'react';
import type { TranslateNS } from '@deepseek-ai/dsh-client-ui-slots';
import { type ExclusionRule } from './search-rules.ts';
/**
 * The settings panel.
 * @param props.rules - the navigator's current draft.
 * @param props.respectGitignore - whether the workspace's search leaves out entries `.gitignore` ignores.
 * @param props.onRespectGitignore - saves a new `.gitignore` choice at once.
 * @param props.trigger - the control that opens and closes the panel; focus moving to it leaves the closing to its click.
 * @param props.onChange - replaces the draft without saving preferences.
 * @param props.onClose - saves and closes the draft, returning focus to the trigger when requested.
 * @param props.t - namespace-bound translate.
 * @returns the panel.
 */
export declare function SearchSettings({ rules, respectGitignore, onRespectGitignore, trigger, onChange, onClose, t }: {
    readonly rules: readonly ExclusionRule[];
    readonly respectGitignore: boolean;
    readonly onRespectGitignore: (respectGitignore: boolean) => void;
    readonly trigger: RefObject<HTMLElement | null>;
    readonly onChange: (rules: readonly ExclusionRule[]) => void;
    readonly onClose: (refocus: boolean) => void;
    readonly t: TranslateNS<'sidebarFiles'>;
}): ReactNode;
//# sourceMappingURL=SearchSettings.d.ts.map