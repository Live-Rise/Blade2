// One-document native editor. The reader owns no VCL objects; all document
// operations and callbacks are pumped on the process initialization thread.
#include <sal/config.h>
#define LOK_USE_UNSTABLE_API
#include <LibreOfficeKit/LibreOfficeKit.h>
#include <LibreOfficeKit/LibreOfficeKitEnums.h>
#include <tools/json_writer.hxx>
#include "lib/init.hxx"
#include <cppuhelper/implbase.hxx>
#include <com/sun/star/util/XModifyBroadcaster.hpp>
#include <com/sun/star/util/XModifyListener.hpp>
#include <boost/property_tree/json_parser.hpp>
#include <vcl/svapp.hxx>
#include <config_features.h>
#if HAVE_FEATURE_SKIA
#include <vcl/skia/SkiaHelper.hxx>
#endif
#include <atomic>
#include <algorithm>
#include <climits>
#include <com/sun/star/uno/Exception.hpp>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <filesystem>
#include <iostream>
#include <memory>
#include <mutex>
#include <sstream>
#include <stdexcept>
#include <thread>
#include <vector>
#include "editor-pixels.hxx"
#include "dsh_frame_forwarding.h"
#ifdef SK_GRAPHITE
#include "graphite-interop-internal.hxx"
#include <skia/dsh_graphite.hxx>
#include <skia/dsh_ganesh.hxx>
#endif
#include <cmath>
#include <map>
#include <set>
#include <functional>

#ifdef SK_GRAPHITE
extern "C" void dsh_native_paint_graphite(LibreOfficeKitDocument*, WGPUTexture, int, int, int, int, int, int);
extern "C" int dsh_native_slideshow_graphite(void*, WGPUTexture, int*);
extern "C" void dsh_native_slideshow_draw_graphite(void*, WGPUTexture);
extern "C" void dsh_native_paint_ganesh(LibreOfficeKitDocument*, const office_ganesh::Target*, int, int, int, int, int, int);
extern "C" int dsh_native_slideshow_ganesh(void*, const office_ganesh::Target*, int*);
extern "C" void dsh_native_slideshow_draw_ganesh(void*, const office_ganesh::Target*);
namespace {
dsh::graphite::detail::InteropContext* graphiteContext = nullptr;
/** Appends the recorded Graphite failure to a fixed message, when the backend recorded one. */
std::string withGraphiteCause(std::string message)
{
    if (const auto cause = office_graphite::failure(); !cause.empty()) message += ": " + cause;
    return message;
}
}
extern "C" void dsh_native_graphite_initialize(unsigned long long cacheBytes)
{
    if (graphiteContext) throw std::runtime_error("Graphite is already initialized");
    char error[512]{};
    dsh::graphite::detail::DawnObjects objects{};
    WGPUDeviceDescriptor descriptor = WGPU_DEVICE_DESCRIPTOR_INIT;
    descriptor.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
    descriptor.deviceLostCallbackInfo.callback = [](WGPUDevice const*, WGPUDeviceLostReason reason, WGPUStringView message, void*, void*) {
        // Dawn's reason and message carry the real Metal or D3D failure, which the fixed error text cannot name.
        const std::string text = message.data ? std::string(message.data, message.length) : std::string();
        office_graphite::deviceLost(reason, text.c_str());
    };
    descriptor.uncapturedErrorCallbackInfo.callback = [](WGPUDevice const*, WGPUErrorType, WGPUStringView message, void*, void*) {
        const std::string text = message.data ? std::string(message.data, message.length) : std::string();
        std::fprintf(stderr, "WebGPU: %s\n", text.c_str());
        office_graphite::deviceError(text.c_str());
    };
    auto* context = dsh::graphite::detail::create_context(&descriptor, &objects, error, sizeof(error));
    if (!context) throw std::runtime_error(error);
    try { office_graphite::initialize(objects.instance, objects.device, objects.queue, static_cast<size_t>(cacheBytes)); }
    catch (...) { dsh::graphite::detail::destroy_context(context); throw; }
    graphiteContext = context;
}
extern "C" void dsh_native_graphite_shutdown()
{
    office_graphite::shutdown();
    dsh::graphite::detail::destroy_context(graphiteContext);
    graphiteContext = nullptr;
}
#else
using WGPUTexture = void*;
extern "C" void dsh_native_graphite_initialize(unsigned long long)
{
    throw std::runtime_error("Graphite requires a macOS or Windows engine");
}
extern "C" void dsh_native_graphite_shutdown() {}
void dsh_native_paint_graphite(LibreOfficeKitDocument*, WGPUTexture, int, int, int, int, int, int)
{
    throw std::runtime_error("Graphite is unavailable in this engine");
}
int dsh_native_slideshow_graphite(void*, WGPUTexture, int*)
{
    throw std::runtime_error("Graphite is unavailable in this engine");
}
void dsh_native_slideshow_draw_graphite(void*, WGPUTexture)
{
    throw std::runtime_error("Graphite is unavailable in this engine");
}
#endif
extern "C" void* dsh_native_slideshow_create(LibreOfficeKitDocument*, int, int, int);
extern "C" void dsh_native_slideshow_destroy(void*);
extern "C" void dsh_native_slideshow_control(void*, int, int);
extern "C" void dsh_native_slideshow_resize(void*, int, int);
extern "C" int dsh_native_slideshow_frame(void*, unsigned char*, int*);
extern "C" int dsh_native_slideshow_frame_bgra(void*, unsigned char*, int*);
extern "C" double dsh_native_slideshow_delay(void*);
extern "C" int dsh_native_slideshow_state(void*, int*);
extern "C" void dsh_native_slideshow_frame_rate(void*, int);
extern "C" bool dsh_native_slideshow_prepare(void*, bool);
extern "C" void dsh_native_slideshow_draw_bgra(void*, unsigned char*);

namespace {
void diagnosticStage(const char* stage)
{
    const char* flag = std::getenv("OFFICE_LOAD_DIAGNOSTICS");
    if (!flag || std::strcmp(flag, "1") != 0) return;
    std::fprintf(stderr, "officeLoad %s\n", stage);
    std::fflush(stderr);
}

constexpr size_t MaxControlBytes = 1024 * 1024;
constexpr size_t MaxQueuedBytes = 8 * MaxControlBytes;
class ModificationListener final : public cppu::WeakImplHelper<css::util::XModifyListener> {
public:
    int64_t revision = 0;
    bool suppressed = true;
    void SAL_CALL modified(const css::lang::EventObject&) override { if (!suppressed) ++revision; }
    void SAL_CALL disposing(const css::lang::EventObject&) override {}
};
struct State {
    std::mutex mutex;
    std::condition_variable changed;
    std::deque<std::string> requests;
    std::deque<std::pair<int, std::string>> events;
    size_t requestBytes = 0, eventBytes = 0;
    bool eof = false, stopped = false;
    std::string failure;
    std::atomic<bool> idle {false};
    int view = -1;
    std::string commandFailure;
};

uint32_t decode(const unsigned char* p)
{
    return (uint32_t(p[0]) << 24) | (uint32_t(p[1]) << 16) | (uint32_t(p[2]) << 8) | p[3];
}

void send(FILE* output, const OString& value, const std::vector<unsigned char>& pixels = {})
{
    const uint32_t lengths[] = { static_cast<uint32_t>(value.getLength()), static_cast<uint32_t>(pixels.size()) };
    unsigned char header[8];
    for (int field = 0; field < 2; ++field)
        for (int byte = 0; byte < 4; ++byte) header[field * 4 + byte] = lengths[field] >> (24 - byte * 8);
    if (std::fwrite(header, 1, 8, output) != 8
        || std::fwrite(value.getStr(), 1, value.getLength(), output) != static_cast<size_t>(value.getLength())
        || (!pixels.empty() && std::fwrite(pixels.data(), 1, pixels.size(), output) != pixels.size())
        || std::fflush(output) != 0) throw std::runtime_error("Editor output closed");
}

void send(FILE* output, tools::JsonWriter& json, const std::vector<unsigned char>& pixels = {})
{
    send(output, json.finishAndGetAsOString(), pixels);
}

void readRequests(const std::shared_ptr<State>& state)
{
    try {
        for (;;) {
            unsigned char header[8];
            std::cin.read(reinterpret_cast<char*>(header), 8);
            if (std::cin.gcount() == 0 && std::cin.eof()) break;
            if (std::cin.gcount() != 8) throw std::runtime_error("Incomplete editor frame header");
            const auto length = decode(header);
            if (length == 0 || length > MaxControlBytes || decode(header + 4) != 0)
                throw std::runtime_error("Invalid editor input frame length");
            std::string value(length, '\0');
            std::cin.read(value.data(), length);
            if (static_cast<uint32_t>(std::cin.gcount()) != length) throw std::runtime_error("Incomplete editor frame");
            std::unique_lock lock(state->mutex);
            state->changed.wait(lock, [&] { return state->stopped || state->requestBytes + length <= MaxQueuedBytes; });
            if (state->stopped) return;
            state->requestBytes += length;
            state->requests.push_back(std::move(value));
            state->changed.notify_all();
        }
    } catch (const std::exception& error) {
        std::lock_guard lock(state->mutex);
        state->failure = error.what();
    }
    std::lock_guard lock(state->mutex);
    state->eof = true;
    state->changed.notify_all();
}

void callback(int type, const char* payload, void* context) noexcept
{
    auto& state = *static_cast<State*>(context);
    if (type == LOK_CALLBACK_UNO_COMMAND_RESULT && payload && std::strstr(payload, "\"editor-barrier\"")) state.idle = true;
    try {
        std::lock_guard lock(state.mutex);
        if (state.stopped || !state.failure.empty()) return;
        if (type == LOK_CALLBACK_UNO_COMMAND_RESULT && payload
            && std::strstr(payload, "\"success\":false")) state.commandFailure = payload;
        const size_t size = (payload ? std::strlen(payload) : 0) + 64;
        if (state.eventBytes + size > MaxQueuedBytes) { state.failure = "Editor callback queue exceeded its byte limit"; return; }
        state.eventBytes += size;
        state.events.emplace_back(type, payload ? payload : "");
    } catch (...) {
        // The C callback cannot propagate allocation failure through Core.
        std::abort();
    }
}

void flushEvents(State& state, FILE* output)
{
    std::deque<std::pair<int, std::string>> events;
    {
        std::lock_guard lock(state.mutex);
        events.swap(state.events);
        state.eventBytes = 0;
    }
    for (const auto& [type, payload] : events) {
        tools::JsonWriter json;
        json.put("kind", "event"); json.put("type", type); json.put("payload", payload);
        send(output, json);
    }
}

void settle(State& state, LibreOfficeKitDocument* document, FILE* output)
{
    state.idle = false;
    document->pClass->setView(document, state.view);
    document->pClass->postUnoCommand(document, ".uno:ReportWhenIdle",
        "{\"idleID\":{\"type\":\"string\",\"value\":\"editor-barrier\"}}", false);
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    while (!state.idle) {
#ifdef SK_GRAPHITE
        office_graphite::retireLostDevice();
#endif
        if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Editor command did not reach idle");
        SolarMutexGuard guard;
        if (!Application::Reschedule(true)) std::this_thread::sleep_for(std::chrono::milliseconds(1));
        flushEvents(state, output);
    }
    document->pClass->setView(document, state.view);
}

std::string take(char* value)
{
    if (!value) return {};
    const std::string result(value);
    std::free(value);
    return result;
}

/** Active Skia drawing backend: `ganesh` for Vulkan or Metal, `raster` for the CPU fallback. */
extern "C" const char* dsh_frame_forwarding_backend()
{
#ifdef SK_GRAPHITE
    if (office_ganesh::initialized()) return "ganesh";
#endif
#if HAVE_FEATURE_SKIA
    // isVCLSkiaEnabled() also probes the GPU by building a temporary window context and forces
    // the raster method when denylisted drivers or a missing context cannot start Skia on GPU.
    return SkiaHelper::isVCLSkiaEnabled() && SkiaHelper::renderMethodToUse() != SkiaHelper::RenderRaster
        ? "ganesh" : "raster";
#else
    return "raster";
#endif
}

// Initialization precedes document drawing, so Raster fallback cannot mix cached GPU images.
extern "C" void dsh_native_ganesh_initialize(bool required)
{
#ifdef SK_GRAPHITE
    if (std::strcmp(dsh_frame_forwarding_backend(), "ganesh") != 0) return;
    try { office_ganesh::initialize(); }
    catch (const std::exception& error) {
        if (required) throw;
        SkiaHelper::forceRasterRenderMethod();
        std::fprintf(stderr, "officeRenderBackendFallback: Ganesh shared-image initialization failed: %s; using raster\n", error.what());
    }
#else
    (void)required;
#endif
}

void info(LibreOfficeKitDocument* document, tools::JsonWriter& json)
{
    long width = 0, height = 0;
    document->pClass->getDocumentSize(document, &width, &height);
    json.put("documentType", document->pClass->getDocumentType(document));
    const char* activeBackend = dsh_frame_forwarding_backend();
#ifdef SK_GRAPHITE
    // Graphite records into Dawn shared textures while VCL keeps its raster Skia method.
    if (graphiteContext) activeBackend = "graphite";
#endif
    json.put("renderBackend", activeBackend);
    json.put("width", static_cast<int64_t>(width)); json.put("height", static_cast<int64_t>(height));
    json.put("part", document->pClass->getPart(document));
    json.put("tileMode", document->pClass->getTileMode(document));
    json.put("rectangles", take(document->pClass->getPartPageRectangles(document)));
    const int count = document->pClass->getParts(document);
    if (count < 1 || count > 10000) throw std::runtime_error("Editor document has too many parts");
    auto parts = json.startArray("parts");
    for (int part = 0; part < count; ++part) {
        auto item = json.startStruct();
        json.put("index", part); json.put("name", take(document->pClass->getPartName(document, part)));
    }
}

int bounded(const boost::property_tree::ptree& value, const char* name, int minimum, int maximum)
{
    const int number = value.get<int>(name);
    if (number < minimum || number > maximum) throw std::runtime_error(std::string("Editor field outside range: ") + name);
    return number;
}

const std::vector<std::string> Commands {
    ".uno:Undo", ".uno:Redo", ".uno:Bold", ".uno:Italic", ".uno:Underline", ".uno:CharFontName", ".uno:FontHeight",
    ".uno:LeftPara", ".uno:CenterPara", ".uno:RightPara", ".uno:JustifyPara", ".uno:DefaultBullet", ".uno:DefaultNumbering",
    ".uno:ULSpacing", ".uno:SelectAll", ".uno:GoToStartOfDoc", ".uno:GoToEndOfDoc", ".uno:GoToCell",
    ".uno:DuplicatePage", ".uno:InsertPage", ".uno:DeletePage",
    ".uno:Cut", ".uno:Delete", ".uno:Copy", ".uno:InsertRowsBefore", ".uno:InsertColumnsBefore", ".uno:DeleteRows", ".uno:DeleteColumns",
};
#include "editor-render.hxx"
}

extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_editor_loading(FILE* output)
{
    tools::JsonWriter event; event.put("kind", "loading"); event.put("stage", "document"); send(output, event);
}

extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_editor_session(
    LibreOfficeKitDocument* document, FILE* output, unsigned long long maxOutputBytes, bool writable)
{
    const auto state = std::make_shared<State>();
    rtl::Reference<ModificationListener> modifications = new ModificationListener;
    css::uno::Reference<css::util::XModifyBroadcaster> broadcaster(
        static_cast<desktop::LibLODocument_Impl*>(document)->mxComponent, css::uno::UNO_QUERY_THROW);
    broadcaster->addModifyListener(modifications);
    int64_t lastRevision = 0;
    std::unique_ptr<void, decltype(&dsh_native_slideshow_destroy)> player(nullptr, dsh_native_slideshow_destroy);
    int slideWidth = 0, slideHeight = 0;
    std::string streamId;
    bool streamFrameRequired = false;
    uint64_t streamGeneration = 0;
    auto nextStreamFrame = std::chrono::steady_clock::time_point::max();
    std::unique_ptr<RenderConnection> render;
    int reportedPart = -1;
    bool reportedEnd = false;
    const auto reportPresentation = [&] {
        if (!player) { reportedPart = -1; return; }
        int part = 0; const bool ended = dsh_native_slideshow_state(player.get(), &part) != 0;
        if (part == reportedPart && ended == reportedEnd) return;
        reportedPart = part; reportedEnd = ended;
        tools::JsonWriter event; event.put("kind", "presentation"); event.put("part", part); event.put("ended", ended); send(output, event);
    };
    // A fatal protocol error may leave stdin open. The isolated helper exits
    // after VCL cleanup; this reader retains only its shared queue until then.
    std::thread(readRequests, state).detach();
    diagnosticStage("editor-initialize-begin");
    document->pClass->initializeForRendering(document, "{}");
    diagnosticStage("editor-initialize-ready");
    state->view = document->pClass->getView(document);
    document->pClass->registerCallback(document, callback, state.get());
    document->pClass->setViewReadOnly(document, document->pClass->getView(document), !writable);
    int previewView = -1;
    // Background slides share the document, but never the user's selection or active part.
    const auto paintView = [&](const boost::property_tree::ptree& value, const auto& draw) {
        if (!value.get_child_optional("part")) { draw(); return; }
        if (document->pClass->getDocumentType(document) != LOK_DOCTYPE_PRESENTATION)
            throw std::runtime_error("A background part requires a presentation");
        const int part = bounded(value, "part", 0, document->pClass->getParts(document) - 1);
        const int activeView = state->view;
        try {
            if (previewView < 0) {
                previewView = document->pClass->createView(document);
                if (previewView < 0) throw std::runtime_error("Cannot create a background presentation view");
                document->pClass->initializeForRendering(document, "{}");
                document->pClass->setViewReadOnly(document, previewView, true);
            } else document->pClass->setView(document, previewView);
            document->pClass->setPart(document, part);
            draw();
        } catch (...) { document->pClass->setView(document, activeView); throw; }
        document->pClass->setView(document, activeView);
    };
    const auto stop = [&] {
        render.reset();
        if (previewView >= 0) {
            const int activeView = state->view;
            document->pClass->destroyView(document, previewView);
            document->pClass->setView(document, activeView);
            previewView = -1;
        }
        broadcaster->removeModifyListener(modifications);
        document->pClass->registerCallback(document, nullptr, nullptr);
        std::lock_guard lock(state->mutex);
        state->stopped = true;
        state->changed.notify_all();
    };
    try {
        if (document->pClass->getDocumentType(document) == LOK_DOCTYPE_TEXT)
            document->pClass->postUnoCommand(document, ".uno:PrintLayout", nullptr, false);
        diagnosticStage("editor-first-idle-begin");
        settle(*state, document, output);
        diagnosticStage("editor-first-idle-ready");
        tools::JsonWriter ready;
        ready.put("kind", "ready"); ready.put("protocol", 4); ready.put("frameForwardingProtocol", DSH_FRAME_FORWARDING_PROTOCOL_VERSION); ready.put("frameForwardingTransferable", true); info(document, ready); send(output, ready);
        modifications->suppressed = false;
        for (;;) {
            // Deferred view activation can outlive a background paint; input belongs to the primary view.
            document->pClass->setView(document, state->view);
#ifdef SK_GRAPHITE
            office_graphite::retireLostDevice();
#endif
            if (render) {
                try { render->pump(); }
                catch (const std::exception&) { /* The generic receiver reports failure; stdio stays available for detach and save. */ }
            }
            if (lastRevision != modifications->revision) {
                lastRevision = modifications->revision;
                tools::JsonWriter changed; changed.put("kind", "revision"); changed.put("revision", lastRevision); send(output, changed);
            }
            const auto streamDue = [&] {
                return !streamId.empty() && player && render && render->isVisible()
                    && (streamGeneration != render->generation() || std::chrono::steady_clock::now() >= nextStreamFrame);
            };
            std::string request;
            {
                std::unique_lock lock(state->mutex);
                if (!state->failure.empty()) throw std::runtime_error(state->failure);
                if (state->requests.empty()) {
                    if (state->eof) break;
                    if (!streamDue()) state->changed.wait_for(lock, std::chrono::milliseconds(5));
                }
                if (!state->requests.empty()) {
                    request = std::move(state->requests.front()); state->requests.pop_front();
                    state->requestBytes -= request.size(); state->changed.notify_all();
                }
            }
            if (request.empty()) {
                SolarMutexGuard guard;
                if (streamDue()) {
                    RenderConnection::Frame* surface = nullptr;
                    try {
                        const int width = render->width(), height = render->height();
                        if (width > 4096 || height > 4096 || static_cast<size_t>(width) * height * 4 > maxOutputBytes)
                            throw std::runtime_error("Shared slideshow viewport exceeds output limit");
                        if (slideWidth != width || slideHeight != height) {
                            dsh_native_slideshow_resize(player.get(), width, height); slideWidth = width; slideHeight = height;
                        }
                        const bool changed = dsh_native_slideshow_prepare(player.get(), streamFrameRequired || streamGeneration != render->generation());
                        const double delay = dsh_native_slideshow_delay(player.get());
                        nextStreamFrame = delay < 0 ? std::chrono::steady_clock::time_point::max()
                            : std::chrono::steady_clock::now() + std::chrono::duration_cast<std::chrono::steady_clock::duration>(std::chrono::duration<double>(delay));
                        if (changed) surface = render->tryAcquire(width, height);
                        if (surface) {
                            std::vector<unsigned char> pixels;
                            if (surface->texture()) dsh_native_slideshow_draw_graphite(player.get(), surface->texture());
#ifdef SK_GRAPHITE
                            else if (surface->ganesh) {
                                const auto target = surface->ganeshTarget(width, height);
                                dsh_native_slideshow_draw_ganesh(player.get(), &target);
                            }
#endif
                            else {
                                pixels.resize(static_cast<size_t>(width) * height * 4);
                                dsh_native_slideshow_draw_bgra(player.get(), pixels.data());
                            }
                            boost::property_tree::ptree metadata;
                            streamGeneration = render->generation(); metadata.put("generation", streamGeneration); metadata.put("requestId", streamId);
                            render->submit(surface, pixels, width, height, metadata, true); surface = nullptr;
                            streamFrameRequired = false;
                        } else if (changed) nextStreamFrame = std::chrono::steady_clock::now() + std::chrono::milliseconds(5);
                        reportPresentation();
                    } catch (const css::uno::Exception& error) {
                        if (surface) render->discard(surface);
                        streamId.clear();
                        try { render->streamFailed(OUStringToOString(error.Message, RTL_TEXTENCODING_UTF8).getStr()); } catch (const std::exception&) { /* The closed receiver reports transport failure. */ }
                    } catch (const std::exception& error) {
                        if (surface) render->discard(surface);
                        streamId.clear();
                        try { render->streamFailed(error.what()); } catch (const std::exception&) { /* A disconnected receiver already reports transport failure. */ }
                    }
                }
                Application::Reschedule(true); flushEvents(*state, output); continue;
            }
            boost::property_tree::ptree value;
            std::istringstream input(request);
            boost::property_tree::read_json(input, value);
            const auto id = value.get<int64_t>("id");
            if (id < 1) throw std::runtime_error("Invalid editor request id");
            const auto operation = value.get<std::string>("op");
            state->commandFailure.clear();
            tools::JsonWriter reply;
            reply.put("kind", "reply"); reply.put("id", id);
            std::vector<unsigned char> pixels;
            try {
#ifdef SK_GRAPHITE
                const bool ganeshLost = office_ganesh::initialized() && !office_ganesh::healthy();
                if ((ganeshLost || (graphiteContext && !office_graphite::healthy()))
                    && operation != "save" && operation != "info" && operation != "selection"
                    && operation != "values" && operation != "inputBarrier"
                    && operation != "detachSurfaceTransport" && operation != "close"
                    && !(operation == "presentation" && value.get<std::string>("action", "") == "stop"))
                    throw std::runtime_error(ganeshLost
                        ? "Ganesh device lost; export the document and retry in a raster process"
                        : withGraphiteCause("Graphite device lost; export the document and retry in a raster process"));
#endif
                const std::vector<std::string> readOperations { "info", "paint", "viewport", "part", "selection", "values", "close", "mouse", "presentation", "attachSurfaceTransport", "detachSurfaceTransport", "paintSurface", "presentationSurface", "inputBarrier" };
                const std::vector<std::string> readCommands { ".uno:SelectAll", ".uno:GoToStartOfDoc", ".uno:GoToEndOfDoc", ".uno:GoToCell", ".uno:Copy" };
                const int keyCode = operation == "key" ? bounded(value, "key", 0, 65535) & 4095 : 0;
                const bool navigation = operation == "key" && bounded(value, "character", 0, 0x10ffff) == 0
                    && ((keyCode >= 1024 && keyCode <= 1031) || keyCode == 1281 || keyCode == 1282);
                if (!writable && !navigation && std::find(readOperations.begin(), readOperations.end(), operation) == readOperations.end()
                    && !(operation == "command" && std::find(readCommands.begin(), readCommands.end(), value.get<std::string>("command")) != readCommands.end()))
                    throw std::runtime_error("Document is read-only");
                modifications->suppressed = (operation != "mouse" && operation != "key" && operation != "composition" && operation != "paste" && operation != "command")
                    || (operation == "command" && std::find(readCommands.begin(), readCommands.end(), value.get<std::string>("command")) != readCommands.end());
                if (player && operation != "presentation" && operation != "presentationSurface" && operation != "attachSurfaceTransport" && operation != "detachSurfaceTransport" && operation != "inputBarrier" && operation != "info" && operation != "close")
                    throw std::runtime_error("Stop the slideshow before editing or changing views");
                if (operation == "attachSurfaceTransport") {
                    if (render) throw std::runtime_error("Frame-forwarding transport is already attached");
                    const auto connectTimeout = bounded(value, "timeoutMs", 1, 2147483647);
                    render = std::make_unique<RenderConnection>(value.get<std::string>("tx"), static_cast<uint32_t>(connectTimeout), document, *state, output, writable, *modifications, [&] { return player != nullptr; });
                } else if (operation == "detachSurfaceTransport") { streamId.clear(); render.reset(); }
                else if (operation == "inputBarrier") {
                    const auto sequence = value.get<uint64_t>("sequence");
                    if (!render && sequence) throw std::runtime_error("Native input transport is not attached");
                    if (render) render->barrier(sequence);
                } else if (operation == "paintSurface" || operation == "presentationSurface") {
                    if (!render) throw std::runtime_error("Frame-forwarding transport is not attached");
                    const bool presentation = operation == "presentationSurface";
                    if (presentation && !player) throw std::runtime_error("Slideshow is not running");
                    const int width = presentation ? slideWidth : bounded(value, "pixelWidth", 1, 4096);
                    const int height = presentation ? slideHeight : bounded(value, "pixelHeight", 1, 4096);
                    if (static_cast<size_t>(width) * height * 4 > maxOutputBytes) throw std::runtime_error("Shared frame exceeds output limit");
                    const auto& metadata = value.get_child("surface");
                    auto surface = render->acquire(width, height);
                    try {
                        paintView(value, [&] {
                        if (!surface->texture() && !surface->ganesh) pixels.resize(static_cast<size_t>(width) * height * 4);
#ifdef SK_GRAPHITE
                        if (surface->ganesh) {
                            const auto target = surface->ganeshTarget(width, height);
                            if (presentation) {
                                int part = 0;
                                reply.put("ended", dsh_native_slideshow_ganesh(player.get(), &target, &part) != 0);
                                reply.put("part", part);
                            } else dsh_native_paint_ganesh(document, &target, width, height,
                                bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX), bounded(value, "width", 1, INT_MAX), bounded(value, "height", 1, INT_MAX));
                        } else
#endif
                        if (presentation) {
                            int part = 0;
                            reply.put("ended", (surface->texture() ? dsh_native_slideshow_graphite(player.get(), surface->texture(), &part)
                                : dsh_native_slideshow_frame_bgra(player.get(), pixels.data(), &part)) != 0); reply.put("part", part);
                        } else if (surface->texture()) {
                            dsh_native_paint_graphite(document, surface->texture(), width, height,
                                bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX), bounded(value, "width", 1, INT_MAX), bounded(value, "height", 1, INT_MAX));
                        } else {
                            document->pClass->paintTile(document, pixels.data(), width, height,
                                bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX), bounded(value, "width", 1, INT_MAX), bounded(value, "height", 1, INT_MAX));
                            const int mode = document->pClass->getTileMode(document);
                            if (mode != 0 && mode != 1) throw std::runtime_error("Unsupported shared tile channel order");
                            if (mode == 0) for (size_t offset = 0; offset < pixels.size(); offset += 4) std::swap(pixels[offset], pixels[offset + 2]);
                        }
                        });
                        render->submit(surface, pixels, width, height, metadata);
                    } catch (...) { render->discard(surface); throw; }
                    reply.put("shared", true); reply.put("requestId", metadata.get<std::string>("requestId"));
                    reply.put("width", width); reply.put("height", height); reply.put("pixelWidth", width); reply.put("pixelHeight", height);
                } else if (operation == "info") info(document, reply);
                else if (operation == "presentation") {
                    const auto action = value.get<std::string>("action");
                    if (action == "stop") { streamId.clear(); player.reset(); }
                    else if (action == "stream" && !value.get<bool>("enabled")) {
                        if (value.get<std::string>("requestId") == streamId) streamId.clear();
                    }
                    else if (action == "start") {
                        if (document->pClass->getDocumentType(document) != LOK_DOCTYPE_PRESENTATION) throw std::runtime_error("Slideshow requires a presentation");
                        const int width = bounded(value, "pixelWidth", 1, 4096), height = bounded(value, "pixelHeight", 1, 4096);
                        if (static_cast<size_t>(width) * height * 4 > maxOutputBytes) throw std::runtime_error("Slideshow frame exceeds output limit");
                        settle(*state, document, output);
                        streamId.clear(); player.reset();
                        player.reset(dsh_native_slideshow_create(document, width, height,
                            bounded(value, "part", 0, document->pClass->getParts(document) - 1)));
                        slideWidth = width; slideHeight = height;
                    } else {
                        if (!player) throw std::runtime_error("Slideshow is not running");
                        if (action == "stream") {
                            if (value.get<bool>("enabled")) {
                                if (!render) throw std::runtime_error("Frame-forwarding transport is not attached");
                                const auto generation = value.get<uint64_t>("generation");
                                if (generation != render->generation()) throw std::runtime_error("Native stream viewport is stale");
                                const auto requestId = value.get<std::string>("requestId");
                                if (requestId.empty() || requestId.size() > 256) throw std::runtime_error("Invalid native stream request id");
                                dsh_native_slideshow_frame_rate(player.get(), bounded(value, "frameRate", 1, 240));
                                streamId = requestId; streamGeneration = generation; streamFrameRequired = true;
                            } else if (value.get<std::string>("requestId") == streamId) streamId.clear();
                        } else if (action == "frame") {
                            pixels.resize(static_cast<size_t>(slideWidth) * slideHeight * 4);
                            int part = 0;
                            reply.put("ended", dsh_native_slideshow_frame(player.get(), pixels.data(), &part) != 0);
                            reply.put("part", part); reply.put("pixelWidth", slideWidth); reply.put("pixelHeight", slideHeight); reply.put("pixelFormat", "rgba-straight");
                        } else if (action == "resize") {
                            const int width = bounded(value, "pixelWidth", 1, 4096), height = bounded(value, "pixelHeight", 1, 4096);
                            if (static_cast<size_t>(width) * height * 4 > maxOutputBytes) throw std::runtime_error("Slideshow frame exceeds output limit");
                            dsh_native_slideshow_resize(player.get(), width, height);
                            slideWidth = width; slideHeight = height;
                        } else if (action == "next") dsh_native_slideshow_control(player.get(), 0, 0);
                        else if (action == "previous") dsh_native_slideshow_control(player.get(), 1, 0);
                        else if (action == "pause") dsh_native_slideshow_control(player.get(), 2, value.get<bool>("paused"));
                        else if (action == "goto") dsh_native_slideshow_control(player.get(), 3, bounded(value, "part", 0, document->pClass->getParts(document) - 1));
                        else throw std::runtime_error("Unknown slideshow action");
                        if (!streamId.empty()) nextStreamFrame = std::chrono::steady_clock::now();
                    }
                }
                else if (operation == "command") {
                    const auto command = value.get<std::string>("command");
                    if (std::find(Commands.begin(), Commands.end(), command) == Commands.end()) throw std::runtime_error("Unsupported editor command");
                    const auto arguments = value.get<std::string>("arguments", "{}");
                    document->pClass->postUnoCommand(document, command.c_str(), arguments.c_str(), true);
                    settle(*state, document, output);
                } else if (operation == "paste") {
                    const auto mime = value.get<std::string>("mime"), text = value.get<std::string>("text");
                    if (mime != "text/plain;charset=utf-8" && mime != "text/html") throw std::runtime_error("Unsupported editor paste type");
                    diagnosticStage("paste-begin");
                    if (!document->pClass->paste(document, mime.c_str(), text.data(), text.size())) throw std::runtime_error("Editor paste failed");
                    diagnosticStage("paste-returned");
                    settle(*state, document, output);
                    diagnosticStage("paste-idle-ready");
                } else if (operation == "key") {
                    const int key = bounded(value, "key", 0, 65535);
                    document->pClass->postKeyEvent(document, bounded(value, "type", 0, 1), bounded(value, "character", 0, 0x10ffff), keyCode == 1281 ? keyCode : key);
                    settle(*state, document, output);
                } else if (operation == "mouse") {
                    document->pClass->postMouseEvent(document, bounded(value, "type", 0, 2), bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX),
                        bounded(value, "count", 1, 3), bounded(value, "buttons", 0, 7), bounded(value, "modifiers", 0, 65535));
                    settle(*state, document, output);
                } else if (operation == "composition") {
                    const auto text = value.get<std::string>("text");
                    diagnosticStage("composition-begin");
                    document->pClass->postWindowExtTextInputEvent(document, 0, bounded(value, "type", 0, 2), text.c_str());
                    diagnosticStage("composition-returned");
                    settle(*state, document, output);
                    diagnosticStage("composition-idle-ready");
                } else if (operation == "viewport") {
                    const int zoom = bounded(value, "pixelSize", 1, 65536), twips = bounded(value, "twipSize", 1, 1000000);
                    if (render) render->viewport(bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX), zoom, twips);
                    document->pClass->setClientZoom(document, zoom, zoom, twips, twips);
                    document->pClass->setClientVisibleArea(document, bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX),
                        bounded(value, "width", 1, INT_MAX), bounded(value, "height", 1, INT_MAX));
                    settle(*state, document, output);
                } else if (operation == "part") {
                    document->pClass->setPart(document, bounded(value, "part", 0, document->pClass->getParts(document) - 1));
                    settle(*state, document, output); info(document, reply);
                } else if (operation == "selection") {
                    const auto mime = value.get<std::string>("mime");
                    if (mime != "text/plain;charset=utf-8" && mime != "text/html") throw std::runtime_error("Unsupported selection type");
                    reply.put("text", take(document->pClass->getTextSelection(document, mime.c_str(), nullptr)));
                } else if (operation == "values") {
                    const auto command = value.get<std::string>("command");
                    if (command != ".uno:SheetGeometryData" && command != ".uno:ViewRowColumnHeaders" && command != ".uno:CharFontName") throw std::runtime_error("Unsupported editor query");
                    std::string query = command;
                    if (command == ".uno:ViewRowColumnHeaders") {
                        query += "?x=" + std::to_string(bounded(value, "x", 0, INT_MAX)) + "&y=" + std::to_string(bounded(value, "y", 0, INT_MAX))
                            + "&width=" + std::to_string(bounded(value, "width", 1, INT_MAX)) + "&height=" + std::to_string(bounded(value, "height", 1, INT_MAX));
                    }
                    reply.put("value", take(document->pClass->getCommandValues(document, query.c_str())));
                } else if (operation == "paint") {
                    const int width = bounded(value, "pixelWidth", 1, 4096), height = bounded(value, "pixelHeight", 1, 4096);
                    pixels.resize(static_cast<size_t>(width) * height * 4);
                    if (pixels.size() > maxOutputBytes) throw std::runtime_error("Editor paint exceeds output limit");
                    paintView(value, [&] {
                        document->pClass->paintTile(document, pixels.data(), width, height, bounded(value, "x", 0, INT_MAX), bounded(value, "y", 0, INT_MAX),
                            bounded(value, "width", 1, INT_MAX), bounded(value, "height", 1, INT_MAX));
                    });
                    dsh::editor::straightRgba(pixels, document->pClass->getTileMode(document));
                    reply.put("pixelWidth", width); reply.put("pixelHeight", height); reply.put("pixelFormat", "rgba-straight");
                } else if (operation == "save") {
                    const auto path = value.get<std::string>("path"), url = value.get<std::string>("url"), format = value.get<std::string>("format");
                    if (format != "docx" && format != "xlsx" && format != "pptx") throw std::runtime_error("Unsupported editor export format");
                    const std::filesystem::path destination(std::u8string(path.begin(), path.end()));
                    if (!destination.is_absolute() || std::filesystem::exists(destination)) throw std::runtime_error("Editor export requires a fresh absolute path");
                    settle(*state, document, output);
                    if (!document->pClass->saveAs(document, url.c_str(), format.c_str(), nullptr)) throw std::runtime_error("Editor export failed");
                    if (std::filesystem::file_size(destination) > maxOutputBytes) { std::filesystem::remove(destination); throw std::runtime_error("Editor export exceeds output limit"); }
                } else if (operation != "close") throw std::runtime_error("Unknown editor operation");
                if (operation == "presentation" || operation == "presentationSurface") reportPresentation();
                if (!state->commandFailure.empty()) throw std::runtime_error(state->commandFailure);
                reply.put("ok", true);
            } catch (const css::uno::Exception& error) { pixels.clear(); reply.put("ok", false); reply.put("error", OUStringToOString(error.Message, RTL_TEXTENCODING_UTF8)); }
            catch (const std::exception& error) { pixels.clear(); reply.put("ok", false); reply.put("error", error.what()); }
            modifications->suppressed = false;
            reply.put("revision", modifications->revision);
            flushEvents(*state, output);
            const OString encoded = reply.finishAndGetAsOString();
            if (static_cast<unsigned long long>(encoded.getLength()) > std::max<unsigned long long>(MaxControlBytes, maxOutputBytes)) {
                tools::JsonWriter error;
                error.put("kind", "reply"); error.put("id", id); error.put("ok", false);
                error.put("error", "Editor response exceeds output limit"); error.put("revision", modifications->revision);
                send(output, error);
            } else send(output, encoded, pixels);
            if (operation == "close") break;
        }
        stop();
    } catch (...) { stop(); throw; }
}
