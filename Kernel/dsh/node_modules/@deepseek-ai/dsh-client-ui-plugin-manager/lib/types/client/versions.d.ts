/**
 * A registry bundle's Versions section and its update dialog. The section summarizes the Host's version check in one
 * row and unfolds into the newer versions; the dialog confirms one update, follows the Host's steps, and reports how
 * the update ended, including the reinstallation of the removed version after a failed installation.
 */
import { type ReactNode } from 'react';
import { type PackageView, type UpdateState, type VersionCheck } from './manager-store.ts';
import { type Translate } from './presentation.ts';
/**
 * The Versions section of an updatable bundle's page. It checks once when the page opens and folds the newer versions
 * behind a summary row.
 */
export declare function VersionSection({ pkg, check, t, busy, onEnsure, onCheck, onUpdate }: {
    readonly pkg: PackageView;
    readonly check: VersionCheck | undefined;
    readonly t: Translate;
    readonly busy: boolean;
    /** Check a package's versions unless a check already started. */
    readonly onEnsure: (packageName: string) => void;
    readonly onCheck: () => void;
    readonly onUpdate: (version: string) => void;
}): ReactNode;
/** The update dialog: confirmation, then the Host's steps, then the outcome. */
export declare function UpdateDialog({ update, title, t, onRun, onClose, onToggleOutput }: {
    readonly update: UpdateState;
    /** The package's display title. */
    readonly title: string;
    readonly t: Translate;
    readonly onRun: () => void;
    readonly onClose: () => void;
    readonly onToggleOutput: () => void;
}): ReactNode;
//# sourceMappingURL=versions.d.ts.map