# @deepseek-ai/libreoffice-kit

English | [中文](README.zh.md)

Convert, recalculate, and directly render local Office documents with prebuilt LibreOffice engines. Use the same API in a server, desktop application, or document-processing job, with configurable fonts, cancellation, and resource limits.

The conversion and native preview entry points can run together in one process, in either initialization order.

Binary `.doc`, `.xls`, and `.ppt` inputs must be OLE compound documents, such as Office 97–2003 files. Renamed RTF/HTML and `.wps` inputs are unsupported. `missingFonts` is empty for binary inputs because their font tables are interpreted by LibreOffice rather than the OOXML inspector.

The Node API version is independent of its platform engine versions. The 0.2.3 candidate selects 0.2.3 macOS and Windows ARM64/x64 engines and a rebuilt portable 0.2.3 WASM engine. `ENGINE_VERSION` and `discoverRuntime().version` identify the Node API; `ENGINE_VERSIONS` lists the exact compatible engine versions. `discoverRuntime().nativePreview` distinguishes native preview support from PDF conversion. Missing native packages select WASM; malformed installed packages reject.

## Installation and usage

Use Node.js 22.19.0 or newer. This command applies after 0.2.3 is published; unpublished candidates must be built and qualified from source first:

```sh
npm install @deepseek-ai/libreoffice-kit@0.2.3
```

npm installs the shared WASM engine and any matching native engine. The adapter prefers a compatible installed native engine, then selects WASM when no native package supports the host or the optional native package is absent. A corrupt installed package rejects with `unavailable`. Conversion failures never switch engines.

```js
import { createConverter } from '@deepseek-ai/libreoffice-kit';

const converter = await createConverter({ timeoutMs: 120_000 });
try {
  const result = await converter.render({
    inputPath: '/private/work/document.docx',
    outputPath: '/private/work/document.pdf',
  });
  console.log(result.backend, result.missingFonts, result.pageCount);
} finally {
  await converter.dispose();
}
```

Native and WASM engines are platform-filtered optional dependencies. Application builders must verify the required engine is installed: native on macOS/Windows, WASM on Linux. A missing required engine rejects `createConverter()` with `unavailable`.

Each converter serializes renders. A render creates a separate native process or Node worker and private profile, so fonts, document state, and failures do not leak into later renders. The deadline begins after acquiring its conversion slot. An `AbortSignal` cancels queued or active work; cancellation and `dispose()` await process or worker exit and scratch cleanup. Disposed converters reject further work.

Converters created through the compatibility `createConverter(options)` API transparently share bounded font metadata and match results with other process-local converters using the same engine and font configuration. Disposing a converter still joins only its own work; metadata remains available to later compatible converters, with at most 16 retained configurations. `maxFontMetadataCacheBytes` bounds retained per-file metadata; `maxFontResolutionEntries` bounds matching entries and defaults to 4096.

For a long-lived service that wants one explicit lifecycle owner for concurrent conversion slots, create one `createConverterFactory(options)` and obtain converters through `factory.create()`. Those converters use a factory-private cache while retaining separate Workers, native helpers, profiles, and output ownership. Dispose the factory to stop and join every converter it created.

`convert()` exports the format named by the output suffix. PDF results from `render()` and `convert()` include `pageCount`, read from the written PDF's page tree; an output whose trailer, catalog, or page tree is unreadable rejects with `invalid-output` and is removed. `recalculate()` refreshes spreadsheet formula results before saving, and `renderImages()` writes a fresh directory containing PNG tiles plus `manifest.json`. Office images are painted directly from one loaded model; PDF images use PDFium. The CLI exposes the same operations through `dsoffice capabilities|convert|recalculate|render`. Worksheet names are exact: a missing image-render selection rejects with `invalid-document`, and a missing CSV selection rejects with `failed`; neither leaves output behind. Recalculation applies to the entire workbook and rejects `--sheet`. A rejected selection leaves the converter available for subsequent operations.

```sh
npm exec -- dsoffice render --input report.docx --output-dir report-pages --pages 1
npm exec -- dsoffice render --input budget.xlsx --output-dir budget-preview --sheet Summary --range A1:D20
npm exec -- dsoffice convert --input report.docx --output report.pdf
npm exec -- dsoffice recalculate --input budget.xlsx --output recalculated.xlsx
```

Right-to-left worksheets produce readable PNG text with column A on the right. A1 ranges and manifest rectangles retain logical column coordinates; each split tile is independently oriented for display.

Conversion workers run the package's shipped JavaScript with an empty `execArgv`; consumer launch flags such as `--input-type=module` are not inherited.

Windows conversion, image-rendering, and persistent-editing helpers use anonymous stdio pipes and inherit the caller's token and Job restrictions. The pipes preserve binary pixel frames, and polling does not occupy libuv worker threads while a helper waits for commands. Closure waits for process exit and output drainage after cancellation and rejected worksheet selections, including when the protocol reader has paused stdout; this transport does not grant additional filesystem or network access.

Native conversion and image-rendering helpers disable printer enumeration and default-printer lookup with `SAL_DISABLE_PRINTERLIST=1` and `SAL_DISABLE_DEFAULTPRINTER=1`. Document previews and exports do not need a physical printer; these child-process settings prevent printer-driver dialogs and network-printer connection waits without changing the system's printer settings.

On Windows, an engine executable or program path of 260 or more characters uses a temporary helper copy and a junction to the installed resources inside the private operation directory. Resources stay in their installation. Conversion, image rendering, and editing retain the helper paths until the child closes, including during startup failure; cleanup then unlinks the junction. A renderer that exits before returning a protocol response rejects with its exit status and captured diagnostics.

On Linux, the native child searches the selected engine's program directory before system paths for shared libraries. Caller-provided `LD_LIBRARY_PATH` and `LD_PRELOAD` are not inherited.

The caller authorizes input access and owns private input/output directories; paths must be absolute and remain unchanged during conversion. Input files must be regular Office files within the configured byte limits. ZIP entry and uncompressed-size limits apply to OOXML; binary DOC/XLS/PPT files use OLE compound containers validated by the LibreOffice importer. Binary formats retain the same conversion deadline and input/output limits. Output creation uses exclusive mode and permissions `0600`; an existing output is never overwritten. Failed or cancelled renders remove newly created outputs. `maxOutputBytes` limits the returned PDF and its read buffer; native temporary disk files can grow until export completes, then oversized PDFs are rejected and deleted before Node reads them. The caller owns successful PDFs and may send their bytes to a browser PDF viewer.

`ConversionError.code` distinguishes `invalid-document`, `unsupported-format`, `input-too-large`, `output-too-large`, `invalid-output`, `timeout`, `unavailable`, and `failed`. These codes survive the worker/native transports. Invalid installation assets reject creation as `unavailable`; they never enable fallback. Filesystem errors such as `EEXIST`, invalid configuration errors, and caller cancellation reasons remain unchanged.

## Native editor

Native views hide all comments and their hit targets. Exporting an original-format snapshot preserves comment text, authors and reply relationships; editing the document does not enable comment viewing.

`@deepseek-ai/libreoffice-kit/editor` opens a persistent DOCX, XLSX, or PPTX document with `openDocument()`. The caller owns authorization, input bytes, rendering, and saved snapshots; `close()` joins the native process. Editing requires `writable: true`. The editor does not expose a browser document model or LibreOffice desktop windows.

`openDocument({ onProgress })` reports `fonts`, `engine`, and `document` when each loading stage begins. Observer failures become warnings. Rendering can start only after the returned promise resolves; the callback does not make LibreOffice import incremental.

Native document loading sends owned bytes over the helper's stdin into a seekable UNO `InputStream` with `private:stream`; it does not create an input document file. Conversion and image rendering read the caller's input path once and pass bytes to both the inspection worker and the native loader. The standard LibreOfficeKit URL-only loader cannot supply this stream: the bundled Core adds a private memory-loading entry. Missing that entry fails instead of writing a fallback file. Profile/font files, save/export outputs, and LibreOffice's internal repair or storage temporaries are separate; this is not a guarantee of zero filesystem activity. WASM uses its in-memory filesystem.

`renderBackend` selects the native drawing backend. The default `"auto"` enables LibreOffice's Skia Ganesh GPU path, which uses Vulkan on Windows and Metal on macOS, and keeps CPU drawing when the driver is denylisted, the GPU context cannot start, or shared-image import fails its initialization probe; `getInfo().renderBackend` reports the backend actually in use, so callers can surface that fallback. `"ganesh"` requires the GPU backend and rejects the open when it is unavailable, `"graphite"` initializes a native Dawn device before loading the document and binds each final output device directly to a borrowed SDK texture, and `"raster"` forces CPU rendering. `graphiteCacheBytes` defaults to 128 MiB, divided between the Graphite context and recorder caches; live/inflight allocations and SDK display textures are outside this cache budget. Forced-backend creation failures reject opening; shared rendering failures do not select another backend or close the document. The owner may export the current in-memory document and explicitly open a raster replacement.

When migrating from 0.1.3 to the 0.2.0 preview, update the Node adapter and its exact platform engines together; engines from 0.1.3 cannot serve the new editor startup options. Existing conversion calls keep their behavior, while editor calls that need the previous CPU path must set `renderBackend: "raster"`; omission now selects `"auto"`. Opt into `graphite` when opening a desktop document and retain the same shared receiver attachment, input ordering and release responsibilities. Changing backends requires a new process: export the current in-memory snapshot, open its replacement with `renderBackend: "raster"` before closing the old editor, and restore the view in the owner. The snapshot preserves document content, not the old undo/redo stack or running slideshow state. If export or replacement initialization fails, retain the old editor and its unsaved document.

Devices without a usable Graphite backend reject initialization; Linux development native engines support raster only. Borrowed WebGPU objects remain in the native document process; no device is passed through Node or a browser. Dawn manages GPU submission and completion, while the native SDK manages shared access across processes, frame credits, and consumer release. Submitting a WebGPU queue alone does not mean a display consumer has finished using an image.

Opening requires editor protocol 4 and frame-forwarding protocol 1 from the installed engine. A missing or incompatible protocol rejects initialization without selecting another engine.

`getSelection()` returns plain text or HTML for the active selection. Native JSON replies use `maxOutputBytes` with a 1 MiB minimum; a reply exceeding that budget rejects the operation while keeping the document open. Escape cancels selection even with modifiers and does not open native command popups.

`setViewport()` sets the actual visible rectangle in twips and the client pixel/twip scale. Writer opens in print layout; zooming and resizing the viewport preserve its page geometry and line breaks. Impress text selection uses the active cell's line-height settings for rendering and hit testing, including merged cells and vertical alignment.

`render()` and `renderSurface()` accept an optional zero-based `part` in the paint rectangle for background presentation slides. These requests use a lazily created, read-only view of the existing document, preserve the active part and selection, and restore the active view before accepting further input. Word pages use their document rectangles. Both operations require slideshow playback to be stopped. Opening still completes native import and pagination before rendering can begin.

`render()` and `presentationFrame()` return tightly packed straight RGBA in a dedicated buffer that later renders never reuse. The native worker normalizes document tiles in place; opaque slideshow frames already use this format. The adapter validates dimensions and the explicit pixel format without copying or converting the returned pixels.

`input()` and `dispatch()` submit ordered input and whitelisted editing commands. `exportSnapshot()` returns original-format bytes with their format and revision; the caller saves them. Opening copies the input bytes, and editing or exporting never overwrites the caller's source. Read-only documents reject mutations and snapshot export. An abort signal ends the document lifetime; call `close()` to await cleanup even after an operation fails.

Desktop applications that need shared images pass a frame-forwarding channel's TX credential to `attachSurfaceTransport(tx, { timeoutMs })`, then use `renderSurface()` or `presentationSurface()`. `timeoutMs` bounds the connection, including a wait while a previous producer of the channel drains, and must be shorter than the document's operation timeout; a busy channel rejects with a message starting `channel-busy`. After `detachSurfaceTransport()` or process exit, another document process can connect to the same channel with the same credential. Replies carry correlation fields while the separate SDK transports the image; the receiver releases each frame after its display consumer finishes. `waitForInput()` orders commands or exports after the requested input sequence. `detachSurfaceTransport()` retains the document. A healthy device can still serve RGBA requests, but detaching does not change the backend and Graphite pixel requests require readback. Raster reads tiles into a CPU buffer and uploads them into the SDK texture. Ganesh reuses LibreOffice's existing GPU context and copies its final image into the SDK texture with Metal blits or Vulkan external-memory copies; Windows allocates that texture on the Vulkan device's matching D3D11 adapter. Ganesh and Graphite shared output skip final-target CPU readback; document features may still need internal bitmap reads or asset uploads.

Continuous presentation streams acquire and publish only when the slideshow requests a canvas update. Static or paused slides retain their last image without consuming additional frame credits. A pending change survives unavailable credits; the next successful frame includes the latest state, including an animation's final frame. Starting a stream or changing its viewport forces a frame. Explicit `presentationFrame()` and `presentationSurface()` calls still return a frame on every request; document views paint only on explicit requests.

## Engines, fonts, and runtime behavior

The Node API reports its release version through `ENGINE_VERSION`; `ENGINE_VERSIONS` pins each platform engine independently. npm installs prepared engines; installation and conversion never compile LibreOffice or download additional engine payloads. Each engine includes its matching source recipes, patches, build information, and third-party license notices under `sources/` and `licenses/`.

Defaults and all options are documented in the shipped TypeScript declarations in `lib/types/index.d.ts`. Font directories use conventional system/user paths for the selected OS. Indexing skips missing or protected sources and propagates other filesystem errors. `fontkit` indexes original font files and selects installed faces and glyph coverage; it does not rewrite fonts. Each operation discovers font candidates again and revalidates cached file identities. It uses one ordered metadata snapshot; matching entries are invalidated when that snapshot changes. A detected font change during conversion rejects the operation and removes its output. Glyph matching queries only requested Unicode scalars and shares decoding across faces in one physical font collection. It retains scalar answers within the operation and releases each parsed file before examining another; it does not enumerate every glyph or cache font bytes across documents. `missingFonts` contains absent families declared in readable document XML, excluding unrelated engine defaults. Missing glyphs without a named missing family are not a complete document accessibility report.

### Engine package resolver

By default the adapter locates engine packages with Node module resolution from its own installation. Applications whose engine files live elsewhere on disk pass `resolveEnginePackage` in the options of `createConverter()`, `createConverterFactory()`, `openDocument({ options })`, and `discoverRuntime()`. `discoverRuntime()` accepts and validates the same options object as `createConverter()`, so one object can serve every entry point and all of them select the same engine. The option type is `EnginePackageResolver = (name: string) => string`:

- `name` is the platform package (`@deepseek-ai/libreoffice-kit-<target>`) when the host has a native target, then `@deepseek-ai/libreoffice-kit-wasm` if WASM is selected.
- Return the absolute path of that package's `package.json` synchronously. A relative path, a missing path, a path that is not a file, or a non-string result is an error; it never falls back to WASM.
- To report that the native package is absent, throw an error whose `code` is `MODULE_NOT_FOUND`, as `require.resolve` does. WASM is then selected through the same resolver. Any other thrown value is an error.
- The resolver alone decides whether its packages exist: the adapter does not probe its own installation for a package the resolver reports missing. Throw a different error to report a present but incomplete package.
- Converters, factories, and `discoverRuntime()` reject these errors as `ConversionError` with code `unavailable` and the original error as `cause`. `openDocument()` rejects with the original error, as it does for its other engine errors.

The adapter still selects the platform target, applies glibc floors, and validates the version pin and engine assets of the resolved package. The resolver runs on the caller's thread and is not passed to conversion workers. An Electron example is in the [packaging guide](https://github.com/deepseek-ai/dsh-libreoffice-kit/blob/master/docs/packaging.md#engine-package-resolver-for-electron-applications).

### Font metadata cache

Both `createConverter()` and `createConverterFactory()` accept:

| Option | Default | Behavior |
| --- | --- | --- |
| `fontMetadataCacheDirectory` | User system cache directory | Absolute directory path, or `false` to disable disk caching. |
| `maxFontMetadataCacheBytes` | 32 MiB | Positive safe integer limiting each cache file read/write and retained per-file metadata per shared configuration. |

The default directory is `~/Library/Caches/libreoffice-kit` on macOS, `%LOCALAPPDATA%/libreoffice-kit/Cache` on Windows (falling back to the user's `AppData/Local`), and `$XDG_CACHE_HOME/libreoffice-kit` on Linux (falling back to `~/.cache`). The single `font-metadata.json` file contains canonical file paths, exact decimal device/inode identifiers, size, modification/change times, and all face metadata, including empty parse results. It contains no font bytes, glyph coverage, parsed font objects, document text, or document matching entries. Keep this user-local file private.

A first run without reusable metadata performs the full original font inspection. Later processes skip inspection only for discovered files with matching identities. Each operation re-enumerates sources, including supplemental Office sources, so additions, deletions, replacements, and directory changes do not require a restart. Candidate order, file budgets, Office filtering, fallback matching, glyph queries, and original font import remain unchanged. A corrupted, incompatible, oversized, or inaccessible cache falls back to source inspection; unreadable source files never reuse stale records. Format, extractor, and pinned fontkit versions must match.

New cache directories and files use private permissions (`0700`/`0600` where supported). Writes close an exclusive temporary file in the same directory before atomic replacement. Unchanged snapshots are not rewritten; snapshots exceeding the byte limit are not retained or written. Independent processes may publish competing complete snapshots, affecting hit rate only. The owning operation cleans up its temporary file after its metadata Worker exits.

Simultaneous operations within one shared configuration share a short-lived metadata Worker. Each waiter keeps its own cancellation and deadline; cancelling one does not cancel the others. The last departing waiter terminates and joins the Worker. Completed scans retain bounded metadata, without a live scan Worker or original font buffers. Setting `fontMetadataCacheDirectory: false` keeps these in-process sharing and source-version checks. Factory caches have their own lifetime and are cleared by `factory.dispose()`.

When `fontDirectories` is omitted on macOS or Windows, default discovery also checks Microsoft Office's bundled/private font directory and the user's Office font cache. Only curated Office-compatible families absent from normal system roots are added, including SimSun and Microsoft YaHei. Explicit `fontDirectories` disables this supplemental discovery.

Exact installed families take priority in font matching, including explicitly requested handwriting or decorative fonts. Default `fontFallbacks` prefer common serif, sans-serif, and monospaced text families and corresponding Simplified Chinese faces, with Carlito for Calibri and Calibri Light, and Caladea for Cambria. Catalog matching retains the weight and italic style supplied by WASM font requests when matching faces are installed. The complete indexed catalog remains available for glyphs absent from the preferred families. Caller-provided groups replace the defaults; `[]` removes these preferences without disabling catalog discovery. WASM uses the same ordered aliases for imported fonts. The shipped option types describe `fontFallbacks`.

Native conversion writes missing-family choices into its private LibreOffice profile. LibreOffice resolves installed originals and its metric-compatible fonts before consulting these choices, so custom groups can produce different substitutions across engines. Native weight and italic selection depend on the engine and the fonts it can discover; native font preloading requests regular faces.

`maxFontFiles` and `maxFontFileBytes` bound font indexing; `maxLoadedFontBytes` bounds original files explicitly imported by this kit per conversion. WASM uses only imported originals and rejects with `unavailable` when no usable fonts are found; install fonts or configure `fontDirectories` before converting in a minimal container. Native macOS and Windows engines can also use OS-managed fonts, so the import limit is not a cap on native total font memory. Font matching and XML work run inside the cancellable worker; no browser font RPC or DOM is involved.

Node WASM image downscaling uses LibreOffice's CPU image filter. Text layout, font matching, and PDF serialization are CPU work as well.

For reproducible comparisons, use identical documents, fonts, DPI, and limits; WASM installations run on Linux. Report engine startup together with conversion time; every render starts a fresh engine. The WASM assets and platform payloads carry their source, license, and integrity manifests.

## Source and license

This package is licensed under [MPL-2.0](LICENSE). The engine packages include `prebuilds.json` integrity inventories, corresponding source recipes and patches in `sources/`, and third-party redistribution notices in `licenses/`.

## Limitations

- Fidelity depends on source formatting, installed fonts, and the selected engine. Missing-font names do not report every missing glyph.
- DOC/DOCX/ODT, XLS/XLSX/ODS, and PPT/PPTX/ODP conversion is supported; direct image rendering additionally accepts PDF. Conversion does not discover system LibreOffice or download engines and fonts.
- Font import and output limits do not bound all native memory or temporary disk use. Native platform engines may resolve fonts differently from WASM.
- Installations from npm use platform-specific optional packages. Applications that bundle engines must retain the complete selected package, including its resources and notices.
- Windows requires the Microsoft Visual C++ v14 Redistributable matching the Node.js architecture (x64 or ARM64); it is not bundled. Use ARM64 Node.js for the Windows ARM64 engine.
- The preview declares macOS and Windows native engines for ARM64 and x64 and a shared Node WASM engine for Linux. Other native platforms are development recipes.

## Frame forwarding SDK

`@deepseek-ai/libreoffice-kit/frame-forwarding` provides the standalone Node producer/receiver and C/C++ SDK for macOS and Windows x64/ARM64. Install with `--omit=optional` when LibreOffice engines are not needed. All four addons and static libraries ship in the main Kit package. `./frame-forwarding/sdk` describes source and library paths, and `./frame-forwarding/verify` checks complete SDK payloads. Installation and normal runtime do not compile or download native code. See the [SDK documentation](../frame-forwarding/README.md) for ownership and explicit diagnostic builds.
