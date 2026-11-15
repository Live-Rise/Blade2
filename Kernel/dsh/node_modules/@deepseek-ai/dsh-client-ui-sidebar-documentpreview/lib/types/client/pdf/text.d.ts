/** PDF.js's viewer owns selection boundaries and copy normalization; the overlay follows its canvas. */
import 'pdfjs-dist';
import type { RenderPdfText } from './document.ts';
/**
 * Create a selectable overlay in a component-owned host.
 * @param host - absolute overlay matching the displayed canvas dimensions.
 * @returns renderer that attaches the layer to `host` once its text is laid out, before its promise resolves;
 *   a cancelled layer is never attached, and cancellation also releases its resize observer and DOM.
 */
export declare function pdfTextRenderer(host: HTMLDivElement): RenderPdfText;
//# sourceMappingURL=text.d.ts.map