/**
 * The label of a changed file in the files menu, the local changes list, and a
 * file's header: its name, then its directory in smaller subdued type, with the
 * complete path in a tooltip.
 */
import type { ReactNode } from 'react';
/**
 * Render a changed file's name and directory. The directory gives up its space
 * before the name does; assistive technology reads the complete path.
 * @param props.path - the file's path in the repository.
 * @param props.tooltip - the hover text, such as both paths of a renamed file; defaults to the path.
 * @param props.className - class of the outer element, which callers size within their row.
 * @returns the label.
 */
export declare function FileLabel({ path, tooltip, className }: {
    readonly path: string;
    readonly tooltip?: string | undefined;
    readonly className?: string | undefined;
}): ReactNode;
//# sourceMappingURL=FileLabel.d.ts.map