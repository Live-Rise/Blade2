// Private one-document process. Protocol output uses a duplicate stdout;
// LibreOffice diagnostics remain on stderr. The Node owner cancels by terminating it.
#ifdef __APPLE__
#include <TargetConditionals.h>
#include <CoreFoundation/CoreFoundation.h>
#include <CoreText/CoreText.h>
#endif
#include <cassert>
#define LOK_USE_UNSTABLE_API
#ifdef DSH_STATIC_LOK
#include <LibreOfficeKit/LibreOfficeKit.h>
extern "C" LibreOfficeKit* libreofficekit_hook_2(const char*, const char*);
extern "C" LibreOfficeKitDocument* dsh_native_document_load_bytes(LibreOfficeKit*, const unsigned char*, int, const char*);
static LibreOfficeKit* lok_init_2(const char* program, const char* profile)
{
    return libreofficekit_hook_2(program, profile);
}
#else
#include <LibreOfficeKit/LibreOfficeKitInit.h>
#endif
#include "../document-operations.hxx"
#include "load-diagnostics.hxx"
#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
#ifdef _WIN32
#include <windows.h>
#include <io.h>
#include <fcntl.h>
#else
#include <unistd.h>
#endif

namespace fs = std::filesystem;
namespace {
struct ConversionError : std::runtime_error {
    const char* code;
    ConversionError(const char* code, const std::string& message) : std::runtime_error(message), code(code) {}
};

struct Request {
    std::string operation = "convert", program, input, output, scratch, profile, format = "pdf", sheet;
    unsigned long long inputBytes = 0;
    std::string inputFormat;
    std::vector<unsigned char> source;
    bool recalculate = false, writable = false;
    std::string renderBackend = "auto";
    unsigned long long graphiteCacheBytes = 128 * 1024 * 1024;
    std::vector<std::string> fonts;
    unsigned long long maxOutput = 0;
    unsigned int resolution = 0;
};

std::string jsonString(const std::string& value)
{
    std::string result = "\"";
    for (unsigned char ch : value) {
        if (ch == '"' || ch == '\\') { result += '\\'; result += ch; }
        else if (ch < 32) {
            char escaped[7];
            std::snprintf(escaped, sizeof escaped, "\\u%04x", ch);
            result += escaped;
        } else result += ch;
    }
    return result + '"';
}

unsigned long long positiveInteger(const std::string& text)
{
    if (text.empty() || !std::all_of(text.begin(), text.end(), [](char c) { return c >= '0' && c <= '9'; }))
        throw std::runtime_error("Expected a positive integer");
    auto value = std::stoull(text);
    if (value == 0) throw std::runtime_error("Expected a positive integer");
    return value;
}

Request parse(const std::vector<std::string>& args)
{
    Request request;
    for (size_t i = 1; i < args.size(); i += 2) {
        if (i + 1 == args.size()) throw std::runtime_error("Missing worker argument value");
        const auto& option = args[i];
        const auto& value = args[i + 1];
        if (option == "--operation") request.operation = value;
        else if (option == "--program-directory") request.program = value;
        else if (option == "--input-path") request.input = value;
        else if (option == "--input-bytes") {
            request.inputBytes = positiveInteger(value);
            if (request.inputBytes > std::numeric_limits<int>::max()) throw std::runtime_error("Document stream exceeds int32");
        }
        else if (option == "--input-format") request.inputFormat = value;
        else if (option == "--output-path") request.output = value;
        else if (option == "--scratch-directory") request.scratch = value;
        else if (option == "--profile-directory") request.profile = value;
        else if (option == "--format") request.format = value;
        else if (option == "--sheet") {
            if (value.empty()) throw std::runtime_error("Worksheet name must be nonempty");
            request.sheet = value;
        }
        else if (option == "--graphite-cache-bytes") {
            request.graphiteCacheBytes = positiveInteger(value);
            if (request.graphiteCacheBytes < 2 || request.graphiteCacheBytes > 9007199254740991ULL)
                throw std::runtime_error("Graphite cache budget is outside the supported integer range");
        }
        else if (option == "--render-backend") {
            if (value != "auto" && value != "ganesh" && value != "graphite" && value != "raster")
                throw std::runtime_error("Unsupported native render backend");
            request.renderBackend = value;
        }
        else if (option == "--writable") {
            if (value != "true" && value != "false") throw std::runtime_error("Expected boolean editing flag");
            request.writable = value == "true";
        }
        else if (option == "--recalculate") {
            if (value != "true" && value != "false") throw std::runtime_error("Expected boolean recalculation flag");
            request.recalculate = value == "true";
        }
        else if (option == "--font-file") request.fonts.push_back(value);
        else if (option == "--max-output-bytes") request.maxOutput = positiveInteger(value);
        else if (option == "--max-image-resolution") {
            auto resolution = positiveInteger(value);
            if (resolution > std::numeric_limits<unsigned int>::max()) throw std::runtime_error("Image resolution exceeds uint32");
            request.resolution = static_cast<unsigned int>(resolution);
        } else throw std::runtime_error("Unknown worker argument: " + option);
    }
    if (request.operation != "convert" && request.operation != "render-images"
        && request.operation != "edit" && request.operation != "probe-editor" && request.operation != "probe-slideshow")
        throw std::runtime_error("Unknown worker operation");
    if ((request.inputBytes != 0) == !request.input.empty()) throw std::runtime_error("Exactly one document input is required");
    if (!request.input.empty() && !fs::u8path(request.input).is_absolute()) throw std::runtime_error("Input path must be absolute");
    if (request.inputBytes && request.inputFormat.empty()) throw std::runtime_error("Document stream format is required");
    for (const auto& path : {request.program, request.profile})
        if (path.empty() || !fs::u8path(path).is_absolute()) throw std::runtime_error("Worker requires absolute paths");
    if (!request.maxOutput || !request.resolution) throw std::runtime_error("Worker limits are required");
    if (request.operation != "convert") {
        if (request.scratch.empty() || !fs::u8path(request.scratch).is_absolute()) throw std::runtime_error("Raster scratch directory must be absolute");
    } else {
        if (request.output.empty() || !fs::u8path(request.output).is_absolute()) throw std::runtime_error("Output path must be absolute");
        const std::vector<std::string> formats = {"pdf", "docx", "odt", "txt", "xlsx", "ods", "csv", "pptx", "odp"};
        if (std::find(formats.begin(), formats.end(), request.format) == formats.end()) throw std::runtime_error("Unsupported output format");
        if (!request.sheet.empty() && request.format != "csv") throw std::runtime_error("Worksheet selection is only supported for CSV");
        if (request.recalculate && request.format != "xlsx" && request.format != "ods") throw std::runtime_error("Recalculation output must be XLSX or ODS");
        if (fs::exists(fs::u8path(request.output))) throw std::runtime_error("Output path already exists");
    }
    return request;
}

std::string pathUtf8(const fs::path& path)
{
    const auto bytes = path.u8string();
    return std::string(bytes.begin(), bytes.end());
}

std::string fileUrl(const std::string& path)
{
    const auto normalized = fs::u8path(path).generic_u8string();
    std::string result = normalized.front() == '/' ? "file://" : "file:///";
    for (unsigned char c : normalized) {
        if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')
            || c == '/' || c == ':' || c == '-' || c == '_' || c == '.' || c == '~') result += c;
        else {
            char encoded[4];
            std::snprintf(encoded, sizeof encoded, "%%%02X", c);
            result += encoded;
        }
    }
    return result;
}

void environment(const char* name, const std::string& value)
{
#ifdef _WIN32
    if (_putenv_s(name, value.c_str()) != 0) throw std::runtime_error("Failed to set worker environment");
#else
    if (setenv(name, value.c_str(), 1) != 0) throw std::runtime_error("Failed to set worker environment");
#endif
}

std::string officeError(LibreOfficeKit* office)
{
    char* value = office->pClass->getError(office);
    std::string message = value && *value ? value : "LibreOffice operation failed";
    office->pClass->freeError(value);
    return message;
}

void registerFont(const std::string& font, LibreOfficeKit* office)
{
    if (!fs::is_regular_file(fs::u8path(font))) throw std::runtime_error("Font is not a regular file");
#ifdef __APPLE__
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(nullptr, reinterpret_cast<const UInt8*>(font.data()), font.size(), false);
    if (!url) throw std::runtime_error("Failed to create font URL");
    CFErrorRef error = nullptr;
    bool registered = CTFontManagerRegisterFontsForURL(url, kCTFontManagerScopeProcess, &error);
    CFRelease(url);
    bool duplicate = error && CFErrorGetCode(error) == kCTFontManagerErrorAlreadyRegistered;
    if (error) CFRelease(error);
    if (!registered && !duplicate) throw std::runtime_error("CoreText rejected font: " + font);
#elif defined(_WIN32)
    if (!AddFontResourceExW(fs::u8path(font).c_str(), FR_PRIVATE, nullptr))
        throw std::runtime_error("Windows rejected font: " + font);
#else
    office->pClass->setOption(office, "addfont", font.c_str());
#endif
}

void prepareEnvironment(const Request& request)
{
#ifdef _WIN32
    // LOK tiles use 96-DPI coordinates, independent of the desktop display scale.
    if (!SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE))
        throw ConversionError("unavailable", "Failed to configure Windows raster DPI");
    if (!SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)
        || !AddDllDirectory(fs::u8path(request.program).c_str()))
        throw ConversionError("unavailable", "Failed to configure Windows library search directories");
    // Headless document layout uses VCL's display printer, not installed print queues.
    environment("SAL_DISABLE_PRINTERLIST", "1");
    environment("SAL_DISABLE_DEFAULTPRINTER", "1");
#endif
    fs::create_directories(fs::u8path(request.profile));
    environment("SAL_LOK_OPTIONS", "unipoll");
    environment("SAL_DISABLE_OPENCL", "1");
    environment("SAL_ACCESSIBILITY_ENABLED", "0");
    environment("LOK_HOST_ALLOWLIST", "^$");
#if !defined(__APPLE__) && !defined(_WIN32)
    const auto fontConfig = fs::u8path(request.profile) / "fonts.conf";
    std::ofstream config;
    config.exceptions(std::ios::badbit | std::ios::failbit);
    config.open(fontConfig);
    config << "<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"fonts.dtd\"><fontconfig></fontconfig>\n";
    config.close();
    environment("FONTCONFIG_FILE", pathUtf8(fontConfig));
#endif
#if defined(__APPLE__) || defined(_WIN32)
    for (const auto& font : request.fonts) registerFont(font, nullptr);
#endif
}

using OfficePtr = std::unique_ptr<LibreOfficeKit, void(*)(LibreOfficeKit*)>;
using DocumentPtr = std::unique_ptr<LibreOfficeKitDocument, void(*)(LibreOfficeKitDocument*)>;

OfficePtr createOffice(const Request& request)
{
    const auto profileUrl = fileUrl(request.profile);
    OfficePtr office(lok_init_2(request.program.c_str(), profileUrl.c_str()),
        [](LibreOfficeKit* value) { if (value) value->pClass->destroy(value); });
    if (!office) throw ConversionError("unavailable", "LibreOfficeKit initialization failed");
    const auto features = LOK_FEATURE_NO_TILED_ANNOTATIONS
        | ((request.operation == "edit" || request.operation == "probe-editor")
            ? LOK_FEATURE_PART_IN_INVALIDATION_CALLBACK : 0);
    office->pClass->setOptionalFeatures(office.get(), features);
#if !defined(__APPLE__) && !defined(_WIN32)
    for (const auto& font : request.fonts) registerFont(font, office.get());
#endif
    return office;
}

DocumentPtr loadDocument(const Request& request, LibreOfficeKit* office)
{
    const auto input = request.source.empty() ? fileUrl(request.input) : std::string("private:stream");
    const char* diagnosticFlag = std::getenv("OFFICE_LOAD_DIAGNOSTICS");
    const bool diagnostics = diagnosticFlag && std::strcmp(diagnosticFlag, "1") == 0;
    const auto report = +[](int type, const char* payload) {
        std::fprintf(stderr, "officeLoad {\"type\":%d,\"payload\":%s}\n", type, jsonString(payload).c_str());
        std::fflush(stderr);
    };
    dsh::LoadDiagnostics loading(office, diagnostics ? report : nullptr);
    if (diagnostics) { std::fputs("officeLoad begin\n", stderr); std::fflush(stderr); }
    LibreOfficeKitDocument* loaded = nullptr;
    if (!request.source.empty()) {
#ifdef DSH_STATIC_LOK
        loaded = dsh_native_document_load_bytes(office, request.source.data(), static_cast<int>(request.source.size()), "Batch=true,EnableMacrosExecution=false");
#else
        char* loadedPath = nullptr;
        void* handle = lok_dlopen(request.program.c_str(), &loadedPath);
        std::free(loadedPath);
        using LoadBytes = LibreOfficeKitDocument* (*)(LibreOfficeKit*, const unsigned char*, int, const char*);
        auto loadBytes = handle ? reinterpret_cast<LoadBytes>(lok_dlsym(handle, "dsh_native_document_load_bytes")) : nullptr;
        if (!loadBytes) throw ConversionError("unavailable", "LibreOffice memory-loading extension is unavailable");
        loaded = loadBytes(office, request.source.data(), static_cast<int>(request.source.size()), "Batch=true,EnableMacrosExecution=false");
#endif
    } else loaded = office->pClass->documentLoadWithOptions(office, input.c_str(), "Batch=true,EnableMacrosExecution=false");
    DocumentPtr document(loaded,
        [](LibreOfficeKitDocument* value) { if (value) value->pClass->destroy(value); });
    if (!document) throw std::runtime_error(officeError(office)
        + (loading.error().empty() ? "" : "; load interaction: " + loading.error()));
    if (diagnostics) { std::fputs("officeLoad ready\n", stderr); std::fflush(stderr); }
    return document;
}

void convert(const Request& request)
{
    prepareEnvironment(request);
    auto office = createOffice(request);
    auto document = loadDocument(request, office.get());
    if (request.recalculate) dsh::recalculate(office.get(), document.get());
    if (request.format == "csv") dsh::selectCsvSheet(office.get(), document.get(), request.sheet.empty() ? nullptr : request.sheet.c_str());
    const auto options = std::string("{\"ReduceImageResolution\":{\"type\":\"boolean\",\"value\":\"true\"},\"MaxImageResolution\":{\"type\":\"long\",\"value\":\"")
        + std::to_string(request.resolution) + "\"},\"ExportBookmarks\":{\"type\":\"boolean\",\"value\":\"true\"}}";
    const auto output = fileUrl(request.output);
    if (!document->pClass->saveAs(document.get(), output.c_str(), request.format.c_str(),
        request.format == "pdf" ? options.c_str() : request.format == "txt" ? "UTF8,LF" : nullptr))
        throw std::runtime_error(officeError(office.get()));
    if (fs::file_size(fs::u8path(request.output)) > request.maxOutput) {
        fs::remove(fs::u8path(request.output));
        throw ConversionError("output-too-large", "Output exceeds maxOutputBytes");
    }
    char header[5] = {};
    std::ifstream(fs::u8path(request.output), std::ios::binary).read(header, sizeof header);
    if (request.format == "pdf" && std::memcmp(header, "%PDF-", sizeof header) != 0)
        throw ConversionError("invalid-output", "LibreOffice did not produce PDF bytes");
}

struct PdfApi {
    using Open = void* (*)(const unsigned char*, int, const char*);
    using Count = int (*)(void*);
    using Size = int (*)(void*, int, int*, int*);
    using Paint = int (*)(void*, unsigned char*, int, int, int, int, int, int, int);
    using Destroy = int (*)(void*);
    void* handle = nullptr;
    Open open = nullptr;
    Count count = nullptr;
    Size size = nullptr;
    Paint paint = nullptr;
    Destroy destroy = nullptr;
};

#ifdef DSH_STATIC_LOK
extern "C" void* dsh_native_pdf_open(const unsigned char*, int, const char*) noexcept;
extern "C" int dsh_native_pdf_page_count(void*) noexcept;
extern "C" int dsh_native_pdf_page_size(void*, int, int*, int*) noexcept;
extern "C" int dsh_native_pdf_paint(void*, unsigned char*, int, int, int, int, int, int, int) noexcept;
extern "C" int dsh_native_pdf_destroy(void*) noexcept;
#endif

PdfApi pdfApi(const Request& request)
{
#ifdef DSH_STATIC_LOK
    (void)request;
    return { nullptr, dsh_native_pdf_open, dsh_native_pdf_page_count, dsh_native_pdf_page_size,
        dsh_native_pdf_paint, dsh_native_pdf_destroy };
#else
    char* loadedPath = nullptr;
    void* handle = lok_dlopen(request.program.c_str(), &loadedPath);
    std::free(loadedPath);
    if (!handle) throw ConversionError("unavailable", "LibreOffice native PDFium library could not be loaded");
    PdfApi result;
    result.handle = handle;
    result.open = reinterpret_cast<PdfApi::Open>(lok_dlsym(handle, "dsh_native_pdf_open"));
    result.count = reinterpret_cast<PdfApi::Count>(lok_dlsym(handle, "dsh_native_pdf_page_count"));
    result.size = reinterpret_cast<PdfApi::Size>(lok_dlsym(handle, "dsh_native_pdf_page_size"));
    result.paint = reinterpret_cast<PdfApi::Paint>(lok_dlsym(handle, "dsh_native_pdf_paint"));
    result.destroy = reinterpret_cast<PdfApi::Destroy>(lok_dlsym(handle, "dsh_native_pdf_destroy"));
    if (!result.open || !result.count || !result.size || !result.paint || !result.destroy)
        throw ConversionError("unavailable", "LibreOffice native PDFium raster API is unavailable");
    return result;
#endif
}

std::string takeString(LibreOfficeKit* office, char* value, const char* message)
{
    if (!value) throw std::runtime_error(message);
    std::string result(value);
    office->pClass->freeError(value);
    return result;
}

void line(FILE* result, const std::string& value)
{
    std::fprintf(result, "%s\n", value.c_str());
    std::fflush(result);
}

void writePixels(const fs::path& path, const std::vector<unsigned char>& pixels)
{
    std::ofstream output(path, std::ios::binary | std::ios::trunc);
    output.exceptions(std::ios::badbit | std::ios::failbit);
    output.write(reinterpret_cast<const char*>(pixels.data()), static_cast<std::streamsize>(pixels.size()));
    output.close();
}

void renderImages(const Request& request, FILE* result)
{
    prepareEnvironment(request);
    auto office = createOffice(request);
    const bool pdf = request.inputBytes ? request.inputFormat == "pdf" : fs::u8path(request.input).extension() == ".pdf";
    DocumentPtr document(nullptr, [](LibreOfficeKitDocument* value) { if (value) value->pClass->destroy(value); });
    PdfApi api;
    void* pdfDocument = nullptr;
    int documentType = -1;
    int tileMode = LOK_TILEMODE_RGBA;
    if (pdf) {
        api = pdfApi(request);
        std::vector<unsigned char> bytes = request.source;
        if (bytes.empty()) {
            std::ifstream input(fs::u8path(request.input), std::ios::binary);
            input.exceptions(std::ios::badbit | std::ios::failbit);
            bytes.assign(std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>());
        }
        pdfDocument = api.open(bytes.data(), static_cast<int>(bytes.size()), "");
        if (!pdfDocument) throw ConversionError("invalid-document", "PDFium could not open the PDF");
        const int count = api.count(pdfDocument);
        if (count < 1 || count > 100000) throw ConversionError("invalid-document", "PDFium returned an invalid page count");
        std::ostringstream ready;
        ready << "{\"ok\":true,\"kind\":\"ready\",\"documentType\":\"pdf\",\"tileMode\":0,\"pages\":[";
        for (int part = 0; part < count; ++part) {
            int width = 0, height = 0;
            if (!api.size(pdfDocument, part, &width, &height)) throw std::runtime_error("PDFium returned invalid page dimensions");
            if (part) ready << ',';
            ready << "{\"width\":" << width << ",\"height\":" << height << '}';
        }
        ready << "]}";
        line(result, ready.str());
    } else {
        document = loadDocument(request, office.get());
        document->pClass->initializeForRendering(document.get(), "{\".uno:ShowBorderShadow\":{\"type\":\"boolean\",\"value\":\"false\"}}");
        documentType = document->pClass->getDocumentType(document.get());
        tileMode = document->pClass->getTileMode(document.get());
        std::ostringstream ready;
        ready << "{\"ok\":true,\"kind\":\"ready\",\"tileMode\":" << tileMode << ',';
        if (documentType == LOK_DOCTYPE_TEXT) {
            ready << "\"documentType\":\"text\",\"writerRectangles\":"
                  << jsonString(takeString(office.get(), document->pClass->getPartPageRectangles(document.get()), "Writer returned no page rectangles"));
        } else if (documentType == LOK_DOCTYPE_PRESENTATION) {
            const int count = document->pClass->getParts(document.get());
            if (count < 1 || count > 100000) throw ConversionError("invalid-document", "Impress returned an invalid slide count");
            document->pClass->setPartMode(document.get(), LOK_PARTMODE_SLIDES);
            ready << "\"documentType\":\"presentation\",\"pages\":[";
            for (int part = 0; part < count; ++part) {
                long width = 0, height = 0;
                document->pClass->setPart(document.get(), part);
                document->pClass->getDocumentSize(document.get(), &width, &height);
                if (part) ready << ',';
                ready << "{\"width\":" << width << ",\"height\":" << height << '}';
            }
            ready << ']';
        } else if (documentType == LOK_DOCTYPE_SPREADSHEET) {
            const int count = document->pClass->getParts(document.get());
            if (count < 1 || count > 100000) throw ConversionError("invalid-document", "Calc returned an invalid sheet count");
            ready << "\"documentType\":\"spreadsheet\",\"sheets\":[";
            for (int part = 0; part < count; ++part) {
                document->pClass->setPart(document.get(), part);
                const auto info = takeString(office.get(), document->pClass->getPartInfo(document.get(), part), "Calc returned no sheet metadata");
                const auto geometry = takeString(office.get(), document->pClass->getCommandValues(document.get(), ".uno:SheetGeometryData"), "Calc returned no sheet geometry");
                if (part) ready << ',';
                ready << "{\"part\":" << part << ",\"info\":" << jsonString(info) << ",\"geometry\":" << jsonString(geometry) << '}';
            }
            ready << ']';
        } else throw ConversionError("invalid-document", "LibreOffice imported the input as an unexpected document type");
        ready << '}';
        line(result, ready.str());
    }

    std::string command;
    while (std::getline(std::cin, command)) {
        if (command == "DONE") {
            if (pdfDocument && !api.destroy(pdfDocument)) throw std::runtime_error("PDFium cleanup failed");
            pdfDocument = nullptr;
            line(result, "{\"ok\":true,\"kind\":\"done\"}");
            return;
        }
        std::istringstream fields(command);
        char kind = 0;
        int index = 0, part = 0, canvasWidth = 0, canvasHeight = 0, x = 0, y = 0, width = 0, height = 0;
        fields >> kind >> index >> part >> canvasWidth >> canvasHeight >> x >> y >> width >> height;
        if (kind != 'P' || !fields || !fields.eof() || index < 1 || canvasWidth < 1 || canvasHeight < 1
            || static_cast<unsigned long long>(canvasWidth) * canvasHeight > 16ULL * 1024 * 1024
            || x < 0 || y < 0 || width < 1 || height < 1)
            throw std::runtime_error("Invalid native raster command");
        std::vector<unsigned char> pixels(static_cast<size_t>(canvasWidth) * canvasHeight * 4);
        if (pdf) {
            if (!api.paint(pdfDocument, pixels.data(), part, canvasWidth, canvasHeight, x, y, width, height))
                throw std::runtime_error("PDFium tile paint failed");
        } else {
            if (part >= 0) document->pClass->setPart(document.get(), part);
            if (documentType == LOK_DOCTYPE_SPREADSHEET) {
                document->pClass->setClientZoom(document.get(), request.resolution * 15, request.resolution * 15, 21600, 21600);
                document->pClass->setClientVisibleArea(document.get(), x, y, width, height);
            }
            if (part < 0) document->pClass->paintTile(document.get(), pixels.data(), canvasWidth, canvasHeight, x, y, width, height);
            else document->pClass->paintPartTile(document.get(), pixels.data(), part, LOK_PARTMODE_SLIDES,
                canvasWidth, canvasHeight, x, y, width, height);
        }
        const auto path = fs::u8path(request.scratch) / ("tile-" + std::to_string(index) + ".rgba");
        writePixels(path, pixels);
        std::ostringstream painted;
        painted << "{\"ok\":true,\"kind\":\"paint\",\"index\":" << index << ",\"path\":"
                << jsonString(pathUtf8(path)) << ",\"width\":" << canvasWidth << ",\"height\":" << canvasHeight << '}';
        line(result, painted.str());
    }
    throw std::runtime_error("Native raster protocol ended before DONE");
}

#ifdef DSH_STATIC_LOK
extern "C" bool dsh_native_process_events();
extern "C" void dsh_native_graphite_initialize(unsigned long long);
extern "C" void dsh_native_graphite_shutdown();
extern "C" const char* dsh_frame_forwarding_backend();
extern "C" void dsh_native_ganesh_initialize(bool);
extern "C" void dsh_native_editor_loading(FILE*);
extern "C" void dsh_native_editor_session(LibreOfficeKitDocument*, FILE*, unsigned long long, bool);
extern "C" bool dsh_native_probe_slideshow(LibreOfficeKitDocument*, unsigned char*, int, int);

// The probe owns one document on the initialization thread. Callbacks only
// record observations; they never re-enter LOK or throw across its C interface.
struct EditorProbe {
    bool idle = false;
    unsigned long callbacks = 0, invalidations = 0;

    static void callback(int type, const char* payload, void* context) noexcept
    {
        auto& probe = *static_cast<EditorProbe*>(context);
        ++probe.callbacks;
        if (type == LOK_CALLBACK_INVALIDATE_TILES) ++probe.invalidations;
        if (type == LOK_CALLBACK_UNO_COMMAND_RESULT && payload
            && std::strstr(payload, "\".uno:ReportWhenIdle\"")
            && std::strstr(payload, "\"native-editor-probe\"")) probe.idle = true;
    }

    void settle(LibreOfficeKitDocument* document)
    {
        idle = false;
        document->pClass->postUnoCommand(document, ".uno:ReportWhenIdle",
            "{\"idleID\":{\"type\":\"string\",\"value\":\"native-editor-probe\"}}", false);
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (!idle) {
            if (std::chrono::steady_clock::now() >= deadline)
                throw std::runtime_error("Native editor idle callback timed out");
            if (!dsh_native_process_events()) std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
    }

    void command(LibreOfficeKitDocument* document, const char* name)
    {
        document->pClass->postUnoCommand(document, name, nullptr, true);
        settle(document);
    }
};

void probeEditor(const Request& request, FILE* result)
{
    prepareEnvironment(request);
    auto office = createOffice(request);
    EditorProbe probe;
    auto document = loadDocument(request, office.get());
    document->pClass->initializeForRendering(document.get(), "{}");
    document->pClass->registerCallback(document.get(), EditorProbe::callback, &probe);
    document->pClass->setClientZoom(document.get(), 256, 256, 3840, 3840);
    document->pClass->setClientVisibleArea(document.get(), 0, 0, 12000, 9000);
    std::vector<unsigned char> initialPixels(800 * 600 * 4);
    document->pClass->paintTile(document.get(), initialPixels.data(), 800, 600, 0, 0, 12000, 9000);
    probe.settle(document.get());
    const int type = document->pClass->getDocumentType(document.get());
    const auto directory = fs::u8path(request.scratch);
    const std::string marker = "NATIVE_EDITOR_CHECK_中文";
    const auto save = [&](const std::string& name, const char* format) {
        const auto path = directory / name;
        if (fs::exists(path)) throw std::runtime_error("Probe output already exists: " + name);
        const char* options = std::strcmp(format, "txt") == 0 ? "UTF8,LF"
            : std::strcmp(format, "csv") == 0 ? "44,34,76,1" : nullptr;
        if (!document->pClass->saveAs(document.get(), fileUrl(pathUtf8(path)).c_str(), format, options))
            throw std::runtime_error(officeError(office.get()));
        if (fs::file_size(path) > request.maxOutput) {
            fs::remove(path);
            throw ConversionError("output-too-large", "Probe output exceeds maxOutputBytes");
        }
    };
    const auto containsMarker = [&](const std::string& name) {
        std::ifstream file(directory / name, std::ios::binary);
        file.exceptions(std::ios::badbit);
        const std::string text((std::istreambuf_iterator<char>(file)), std::istreambuf_iterator<char>());
        return text.find(marker) != std::string::npos;
    };
    if (type == LOK_DOCTYPE_TEXT || type == LOK_DOCTYPE_SPREADSHEET) {
        if (type == LOK_DOCTYPE_TEXT) probe.command(document.get(), ".uno:GoToEndOfDoc");
        if (!document->pClass->paste(document.get(), "text/plain;charset=utf-8", marker.data(), marker.size()))
            throw std::runtime_error("Native editor paste failed");
        probe.settle(document.get());
        const char* textFormat = type == LOK_DOCTYPE_TEXT ? "txt" : "csv";
        save("inserted.txt", textFormat);
        if (!containsMarker("inserted.txt")) throw std::runtime_error("Native editor lost inserted text");
        probe.command(document.get(), ".uno:Undo");
        save("undone.txt", textFormat);
        if (containsMarker("undone.txt")) throw std::runtime_error("Native editor undo retained inserted text");
        probe.command(document.get(), ".uno:Redo");
        save("redone.txt", textFormat);
        if (!containsMarker("redone.txt")) throw std::runtime_error("Native editor redo lost inserted text");
    } else if (type == LOK_DOCTYPE_PRESENTATION) {
        const int before = document->pClass->getParts(document.get());
        probe.command(document.get(), ".uno:DuplicatePage");
        if (document->pClass->getParts(document.get()) != before + 1)
            throw std::runtime_error("Native editor did not duplicate the slide");
        probe.command(document.get(), ".uno:Undo");
        if (document->pClass->getParts(document.get()) != before)
            throw std::runtime_error("Native editor did not undo slide duplication");
        probe.command(document.get(), ".uno:Redo");
        if (document->pClass->getParts(document.get()) != before + 1)
            throw std::runtime_error("Native editor did not redo slide duplication");
    } else throw std::runtime_error("Native editor probe requires Writer, Calc or Impress");

    const std::string format = type == LOK_DOCTYPE_TEXT ? "docx" : type == LOK_DOCTYPE_SPREADSHEET ? "xlsx" : "pptx";
    save("edited." + format, format.c_str());
    const auto callbackDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (probe.invalidations == 0 && std::chrono::steady_clock::now() < callbackDeadline) {
        dsh_native_process_events(); std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    line(result, "{\"ok\":true,\"kind\":\"editing\",\"format\":" + jsonString(format)
        + ",\"callbacks\":" + std::to_string(probe.callbacks) + ",\"invalidations\":" + std::to_string(probe.invalidations) + '}');

    if (type == LOK_DOCTYPE_TEXT) {
        const auto observe = [&](const char* layout, int width) {
            document->pClass->setClientZoom(document.get(), 256, 256, 3840, 3840);
            document->pClass->setClientVisibleArea(document.get(), 0, 0, width * 15, 9000);
            probe.settle(document.get());
            long documentWidth = 0, documentHeight = 0;
            document->pClass->getDocumentSize(document.get(), &documentWidth, &documentHeight);
            std::vector<unsigned char> pixels(static_cast<size_t>(width) * 600 * 4);
            document->pClass->paintTile(document.get(), pixels.data(), width, 600, 0, 0, width * 15, 9000);
            writePixels(directory / (std::string(layout) + '-' + std::to_string(width) + ".rgba"), pixels);
            line(result, "{\"ok\":true,\"kind\":\"layout\",\"layout\":" + jsonString(layout)
                + ",\"viewportWidth\":" + std::to_string(width) + ",\"documentWidth\":" + std::to_string(documentWidth)
                + ",\"documentHeight\":" + std::to_string(documentHeight) + ",\"tileMode\":"
                + std::to_string(document->pClass->getTileMode(document.get())) + '}');
        };
        for (int width : {400, 800, 1200}) observe("print", width);
    }

    const int expectedParts = document->pClass->getParts(document.get());
    document->pClass->registerCallback(document.get(), nullptr, nullptr);
    document.reset();
    Request reopen = request;
    reopen.input = pathUtf8(directory / ("edited." + format));
    reopen.source.clear();
    reopen.inputBytes = 0;
    document = loadDocument(reopen, office.get());
    document->pClass->initializeForRendering(document.get(), "{}");
    document->pClass->registerCallback(document.get(), EditorProbe::callback, &probe);
    document->pClass->setClientZoom(document.get(), 256, 256, 3840, 3840);
    document->pClass->setClientVisibleArea(document.get(), 0, 0, 12000, 9000);

    document->pClass->paintTile(document.get(), initialPixels.data(), 800, 600, 0, 0, 12000, 9000);
    probe.settle(document.get());
    if (type == LOK_DOCTYPE_PRESENTATION) {
        if (document->pClass->getParts(document.get()) != expectedParts)
            throw std::runtime_error("Reopened presentation lost edited slides");
    } else {
        save("reopened.txt", type == LOK_DOCTYPE_TEXT ? "txt" : "csv");
        if (!containsMarker("reopened.txt")) throw std::runtime_error("Reopened document lost inserted text");
    }
    document->pClass->registerCallback(document.get(), nullptr, nullptr);
    line(result, "{\"ok\":true,\"kind\":\"reopened\",\"format\":" + jsonString(format) + '}');
}

void probeSlideshow(const Request& request, FILE* result)
{
    prepareEnvironment(request);
    auto office = createOffice(request);
    auto document = loadDocument(request, office.get());
    document->pClass->initializeForRendering(document.get(), "{}");
    if (document->pClass->getDocumentType(document.get()) != LOK_DOCTYPE_PRESENTATION)
        throw std::runtime_error("Native slideshow probe requires Impress");
    long documentWidth = 0, documentHeight = 0;
    document->pClass->getDocumentSize(document.get(), &documentWidth, &documentHeight);
    if (documentWidth <= 0 || documentHeight <= 0) throw std::runtime_error("Invalid slide dimensions");
    const double scale = std::min(1280.0 / documentWidth, 720.0 / documentHeight);
    const int width = std::max(1, static_cast<int>(documentWidth * scale));
    const int height = std::max(1, static_cast<int>(documentHeight * scale));
    const size_t frameBytes = static_cast<size_t>(width) * height * 4;
    if (frameBytes * 4 > request.maxOutput) throw std::runtime_error("Slideshow probe exceeds maxOutputBytes");
    std::vector<unsigned char> frames(frameBytes * 4);
    if (!dsh_native_probe_slideshow(document.get(), frames.data(), width, height))
        throw std::runtime_error("Native offscreen slideshow failed; inspect stderr");
    for (int index = 0; index < 4; ++index) {
        const std::vector<unsigned char> pixels(frames.begin() + index * frameBytes, frames.begin() + (index + 1) * frameBytes);
        writePixels(fs::u8path(request.scratch) / ("frame-" + std::to_string(index) + ".rgba"), pixels);
    }
    line(result, "{\"ok\":true,\"kind\":\"slideshow-frames\",\"width\":" + std::to_string(width)
        + ",\"height\":" + std::to_string(height) + ",\"frames\":4}");
}
#endif

int execute(const std::vector<std::string>& args)
{
#ifdef _WIN32
    int resultFd = _dup(_fileno(stdout));
    if (resultFd < 0 || _dup2(_fileno(stderr), _fileno(stdout)) != 0) return 2;
    // _fdopen preserves the descriptor's translation mode; pixel bytes require binary output.
    if (_setmode(resultFd, _O_BINARY) == -1) return 2;
    FILE* result = _fdopen(resultFd, "wb");
    _setmode(_fileno(stdin), _O_BINARY);
#else
    int resultFd = dup(fileno(stdout));
    if (resultFd < 0 || dup2(fileno(stderr), fileno(stdout)) < 0) return 2;
    FILE* result = fdopen(resultFd, "w");
#endif
    if (!result) return 2;
    FILE* errors = result;
    int code = 0;
    try {
        auto request = parse(args);
        if (request.inputBytes) {
            request.source.resize(static_cast<size_t>(request.inputBytes));
            std::cin.read(reinterpret_cast<char*>(request.source.data()), static_cast<std::streamsize>(request.source.size()));
            if (!std::cin) throw std::runtime_error("Truncated document input stream");
        }
        if (request.operation == "edit") errors = stderr;
        if (request.operation == "render-images") renderImages(request, result);
        else if (request.operation == "edit") {
#ifdef DSH_STATIC_LOK
            prepareEnvironment(request);
            environment("SAL_ENABLESKIA", "1");
            // Skia GPU drawing needs the platform VCL plugin; the Core init patch selects it from
            // this value. Graphite keeps the headless raster plugin and records through Dawn instead.
            // Skia decides denylisted drivers and missing GPU contexts itself, so `auto` falls back
            // to raster while `ganesh` fails below on the backend Skia actually resolved.
            const bool gpuSkia = request.renderBackend == "auto" || request.renderBackend == "ganesh";
#if defined(__APPLE__)
            environment("SAL_SKIA", gpuSkia ? "metal" : "raster");
#else
            environment("SAL_SKIA", gpuSkia ? "vulkan" : "raster");
#endif
            struct GraphiteLifetime {
                bool active;
                explicit GraphiteLifetime(bool enabled, unsigned long long cacheBytes) : active(enabled) { if (active) dsh_native_graphite_initialize(cacheBytes); }
                ~GraphiteLifetime() { if (active) dsh_native_graphite_shutdown(); }
            } graphite(request.renderBackend == "graphite", request.graphiteCacheBytes);
            auto office = createOffice(request);
            if (gpuSkia) {
                try { dsh_native_ganesh_initialize(request.renderBackend == "ganesh"); }
                catch (const std::exception& error) { throw ConversionError("unavailable", error.what()); }
            }
            if (gpuSkia && std::strcmp(dsh_frame_forwarding_backend(), "ganesh") != 0) {
                // The driver denylist, a missing GPU context, or a build without Skia disabled GPU
                // drawing. `ganesh` fails instead of degrading; `auto` keeps CPU drawing and reports it.
                if (request.renderBackend == "ganesh")
                    throw ConversionError("unavailable", "Skia GPU rendering is unavailable on this device");
                std::fprintf(stderr, "officeRenderBackendFallback: requested auto Skia GPU, using raster\n");
            }
            dsh_native_editor_loading(result);
            auto document = loadDocument(request, office.get());
            dsh_native_editor_session(document.get(), result, request.maxOutput, request.writable);
#else
            throw std::runtime_error("Native editing requires a static Core build");
#endif
        }
        else if (request.operation == "probe-editor" || request.operation == "probe-slideshow") {
#ifdef DSH_STATIC_LOK
            if (request.operation == "probe-editor") probeEditor(request, result);
            else probeSlideshow(request, result);
#else
            throw std::runtime_error("Native editor probe requires a static Core build");
#endif
        }
        else {
            convert(request);
            line(result, "{\"ok\":true,\"missingFonts\":[]}");
        }
    } catch (const ConversionError& error) {
        line(errors, std::string("{\"ok\":false,\"code\":") + jsonString(error.code) + ",\"error\":" + jsonString(error.what()) + '}');
        code = 1;
    } catch (const std::exception& error) {
        line(errors, std::string("{\"ok\":false,\"code\":\"failed\",\"error\":") + jsonString(error.what()) + '}');
        code = 1;
    } catch (...) {
        line(errors, "{\"ok\":false,\"code\":\"failed\",\"error\":\"Unknown LibreOfficeKit exception\"}");
        code = 1;
    }
    std::fclose(result);
    std::_Exit(code);
}
}

#ifdef _WIN32
int wmain(int argc, wchar_t** argv)
{
    std::vector<std::string> args;
    for (int i = 0; i < argc; ++i) {
        const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS,
            argv[i], -1, nullptr, 0, nullptr, nullptr);
        if (length <= 0) return 2;
        std::string text(length, '\0');
        if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS,
                argv[i], -1, text.data(), length, nullptr, nullptr)) return 2;
        text.pop_back();
        args.push_back(std::move(text));
    }
    return execute(args);
}
#else
#ifdef DSH_STATIC_LOK
// SAL locates the executable with dlsym("main") on macOS.
__attribute__((visibility("default")))
#endif
int main(int argc, char** argv)
{
    return execute({argv, argv + argc});
}
#endif
