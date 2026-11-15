// Graphite owns the rendering recorder; DSH supplies the actual WebGPU objects.
#include <skia/dsh_graphite.hxx>
#include <skia/dsh_ganesh.hxx>
#include <skia/gdiimpl.hxx>
#include <skia/utils.hxx>
#include <salgdi.hxx>
#include <vcl/virdev.hxx>
#include <stdexcept>
#include <SkBitmap.h>
#include <SkCanvas.h>
#include <SkPaint.h>
#include <graphite/BackendTexture.h>
#include <graphite/Context.h>
#include <graphite/ContextOptions.h>
#include <graphite/Image.h>
#include <graphite/ImageProvider.h>
#include <graphite/Recorder.h>
#include <graphite/Recording.h>
#include <graphite/Surface.h>
#include <graphite/dawn/DawnBackendContext.h>
#include <graphite/dawn/DawnGraphiteTypes.h>
#include <src/image/SkImage_Base.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>

namespace office_graphite {
namespace {
std::unique_ptr<skgpu::graphite::Context> context;
std::unique_ptr<skgpu::graphite::Recorder> recorder;
std::mutex failureMutex;
bool failed = false;
bool shuttingDown = false;
std::string failureDetail;
std::string lastDeviceError;
size_t surfaces = 0;
size_t readbacks = 0;
size_t submissions = 0;
size_t imageUploads = 0;
size_t directTargets = 0;
const SkSurface* outputTarget = nullptr;
size_t outputSnapshots = 0;
size_t outputReadbacks = 0;
// The first failure is the root cause: later observations only confirm that the device stayed unusable.
void recordFailure(std::string detail) {
    std::lock_guard lock(failureMutex);
    if (failed) return;
    failed = true;
    failureDetail = std::move(detail);
    // Distinct from the `officeGraphite` counter records that qualification helpers parse as numeric-only.
    std::fprintf(stderr, "officeGraphiteFailure: %s\n", failureDetail.c_str());
}
bool isFailed() {
    std::lock_guard lock(failureMutex);
    return failed;
}
void require(bool ok, const char* operation) {
    if (ok) return;
    std::string message = std::string("Graphite rendering failed: ") + operation;
    if (const auto cause = failure(); !cause.empty()) message += ": " + cause;
    throw std::runtime_error(message);
}
const char* deviceLostReasonName(WGPUDeviceLostReason reason) {
    switch (reason) {
        case WGPUDeviceLostReason_Unknown: return "unknown";
        case WGPUDeviceLostReason_Destroyed: return "destroyed";
        default: return "unrecognized";
    }
}
class BitmapImages final : public skgpu::graphite::ImageProvider {
public:
    sk_sp<SkImage> findOrCreate(skgpu::graphite::Recorder* owner, const SkImage* image,
                                SkImage::RequiredProperties properties) override {
        auto texture = SkImages::TextureFromImage(owner, image, properties);
        if (!texture) { recordFailure("image upload to the Graphite recorder failed"); return nullptr; }
        ++imageUploads;
        return texture;
    }
};
}
void initialize(WGPUInstance instance, WGPUDevice device, WGPUQueue queue, size_t cacheBytes) {
    require(!context && instance && device && queue, "initialize supplied WebGPU objects");
    require(!isFailed(), "device lost during initialization");
    skgpu::graphite::DawnBackendContext backend;
    backend.fInstance = wgpu::Instance(instance);
    backend.fDevice = wgpu::Device(device);
    backend.fQueue = wgpu::Queue(queue);
    require(cacheBytes >= 2, "cache budget must be at least two bytes");
    skgpu::graphite::ContextOptions contextOptions;
    contextOptions.fGpuBudgetInBytes = cacheBytes / 2;
    context = skgpu::graphite::ContextFactory::MakeDawn(backend, contextOptions);
    require(bool(context), "create context");
    skgpu::graphite::RecorderOptions options;
    options.fImageProvider = sk_make_sp<BitmapImages>();
    options.fGpuBudgetInBytes = cacheBytes - cacheBytes / 2;
    recorder = context->makeRecorder(options);
    require(bool(recorder), "create recorder");
}
void shutdown() {
    {
        std::lock_guard lock(failureMutex);
        shuttingDown = true;
    }
    recorder.reset();
    context.reset();
}
bool enabled() { return bool(context); }
bool healthy() { return context && !isFailed(); }
std::string failure() {
    std::lock_guard lock(failureMutex);
    return failureDetail;
}
void deviceError(const char* message) {
    if (!message || !*message) return;
    {
        std::lock_guard lock(failureMutex);
        lastDeviceError = message;
    }
    recordFailure(std::string("WebGPU device error: ") + message);
}
void deviceLost(WGPUDeviceLostReason reason, const char* message) {
    std::string detail;
    {
        std::lock_guard lock(failureMutex);
        if (shuttingDown && reason == WGPUDeviceLostReason_Destroyed) return;
        detail = std::string("WebGPU device lost (") + deviceLostReasonName(reason) + ")";
        // A device can be lost without a message; the last uncaptured error then names the cause.
        const char* cause = message && *message ? message : lastDeviceError.c_str();
        if (*cause) detail += std::string(": ") + cause;
    }
    recordFailure(std::move(detail));
}
void retireLostDevice() { if (isFailed()) recorder.reset(); }
bool isGraphite(const sk_sp<SkImage>& image) { return image && as_IB(image)->isGraphiteBacked(); }
sk_sp<SkSurface> createSurface(const SkImageInfo& info, const SkSurfaceProps* properties) {
    // Internal previews may continue while shared output remains explicitly failed.
    if (isFailed()) { retireLostDevice(); return SkSurfaces::Raster(info, properties); }
    auto surface = SkSurfaces::RenderTarget(recorder.get(), info, skgpu::Mipmapped::kNo, properties);
    require(bool(surface), "create VCL Graphite surface");
    require(surface->getCanvas()->recorder() == recorder.get(), "VCL recorder identity");
    ++surfaces;
    return surface;
}
sk_sp<SkSurface> wrapTexture(WGPUTexture texture) {
    auto backend = skgpu::graphite::BackendTextures::MakeDawn(texture);
    auto surface = SkSurfaces::WrapBackendTexture(recorder.get(), backend, nullptr, SkiaHelper::surfaceProps());
    require(bool(surface), "wrap display current texture");
    return surface;
}
bool flush(bool wait) {
    if (!context) return true;
    if (isFailed()) return false;
    auto recording = recorder->snap();
    if (!recording) {
        recordFailure("recording Graphite work failed");
        return false;
    }
    if (isFailed()) return false;
    skgpu::graphite::InsertRecordingInfo insert;
    insert.fRecording = recording.get();
    if (context->insertRecording(insert) != skgpu::graphite::InsertStatus::kSuccess) {
        recordFailure("inserting a Graphite recording failed");
        return false;
    }
    if (!context->submit(wait ? skgpu::graphite::SyncToCpu::kYes : skgpu::graphite::SyncToCpu::kNo)) {
        recordFailure("submitting Graphite work failed");
        return false;
    }
    ++submissions;
    return !isFailed();
}
sk_sp<SkImage> rasterImage(sk_sp<SkImage> image) {
    if (!isGraphite(image)) return image;
    require(flush(false), "GPU unavailable for bitmap readback");
    std::unique_ptr<const SkImage::AsyncReadResult> pixels;
    context->asyncRescaleAndReadPixels(image.get(), image->imageInfo(),
        SkIRect::MakeSize(image->dimensions()), SkImage::RescaleGamma::kSrc,
        SkImage::RescaleMode::kNearest,
        [](void* output, std::unique_ptr<const SkImage::AsyncReadResult> result) {
            *static_cast<std::unique_ptr<const SkImage::AsyncReadResult>*>(output) = std::move(result);
        }, &pixels);
    require(context->submit(skgpu::graphite::SyncToCpu::kYes), "submit diagnostic/bitmap readback");
    require(bool(pixels), "complete diagnostic/bitmap readback");
    SkBitmap bitmap;
    require(bitmap.tryAllocPixels(image->imageInfo()), "allocate bitmap readback");
    for (int y = 0; y < image->height(); ++y)
        std::memcpy(bitmap.getAddr(0, y), static_cast<const char*>(pixels->data(0)) + y * pixels->rowBytes(0), bitmap.rowBytes());
    bitmap.setImmutable();
    ++readbacks;
    return SkImages::RasterFromBitmap(bitmap);
}
bool readSurface(sk_sp<SkSurface> surface, SkBitmap& bitmap) {
    office_ganesh::observeReadback(surface.get());
    if (surface.get() == outputTarget) ++outputReadbacks;
    if (!surface->getCanvas()->recorder()) return surface->readPixels(bitmap, 0, 0);
    auto image = rasterImage(surface->makeImageSnapshot());
    return image->readPixels(bitmap.pixmap(), 0, 0);
}
Target::Target(VirtualDevice& device, WGPUTexture texture) {
    require(healthy(), "Graphite backend is unavailable");
    auto* graphics = device.GetGraphics();
    implementation = graphics ? dynamic_cast<SkiaSalGraphicsImpl*>(graphics->GetImpl()) : nullptr;
    require(implementation != nullptr, "output VirtualDevice must use Skia");
    auto surface = wrapTexture(texture);
    require(outputTarget == nullptr, "nested output target binding");
    const auto* identity = surface.get();
    implementation->bindGraphiteTarget(std::move(surface));
    outputTarget = identity;
    ++directTargets;
}
Target::~Target() {
    if (!implementation) return;
    // Cancellation still submits recorded references before the SDK ends texture access.
    try { flush(false); }
    catch (const std::exception& error) { recordFailure(std::string("output target cleanup failed: ") + error.what()); }
    outputTarget = nullptr;
    implementation->releaseGraphiteTarget();
}
void Target::finish() {
    require(implementation != nullptr, "output target already released");
    implementation->finishGraphiteTarget();
    require(flush(false), "submit output target");
    outputTarget = nullptr;
    implementation->releaseGraphiteTarget();
    implementation = nullptr;
}
void observeSnapshot(const SkSurface* surface) { if (surface == outputTarget) ++outputSnapshots; }
void report() {
    std::fprintf(stderr, "officeGraphite surfaces=%zu readbacks=%zu submissions=%zu imageUploads=%zu directTargets=%zu outputSnapshots=%zu outputReadbacks=%zu\n", surfaces, readbacks, submissions, imageUploads, directTargets, outputSnapshots, outputReadbacks);
    if (const auto cause = failure(); !cause.empty()) std::fprintf(stderr, "officeGraphiteFailure: %s\n", cause.c_str());
}
}
