/**
 * Plugin detail documents without state: language chains, the `meta.detail` or README selection of a package, image
 * destinations beside a document, and read-only file sources for local packages and published npm versions.
 * @module @deepseek-ai/dsh-plugin-detail
 */
export { DetailFailure, encodePath, fetchBody, joinPath, localFiles, NPM_FILE_SOURCES, npmFileSourceOrder, npmFiles, README_NAMES, readmeByName, readmeName, } from "./files.js";
export { detailImage, languageChain, markdownOf, readPackageDocument } from "./document.js";
export { detailImageResponse } from "./response.js";
//# sourceMappingURL=index.js.map