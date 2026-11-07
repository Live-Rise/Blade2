import type { DesktopFileActions, FileActions, FileActionSource, FileSaveResult } from '../types.ts';
/**
 * Keep compatible filename suffixes and repair known MIME conflicts without changing bytes.
 * @param name - suggested original name.
 * @param mediaType - source MIME, optionally including parameters.
 * @returns safe filename preserving unknown formats.
 */
export declare function exportName(name: string | undefined, mediaType: string): string;
/** File action owner; browser download requests never imply a saved path. */
export declare class BrowserFileActions implements FileActions {
    private readonly desktop;
    private downloadUrl;
    private readonly lifetime;
    readonly canSaveAs: boolean;
    constructor(desktop: DesktopFileActions | undefined);
    private read;
    save(source: FileActionSource, mode: 'download' | 'saveAs'): Promise<FileSaveResult>;
    copyImage(source: FileActionSource): Promise<void>;
    /** Cancel reads and release browser download URLs when the plugin unloads. */
    dispose(): void;
}
//# sourceMappingURL=files.d.ts.map