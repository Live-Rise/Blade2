// Process-local Graphite rendering for native document and slideshow output.
#pragma once
#include <SkImage.h>
#include <SkSurface.h>
#include <webgpu/webgpu.h>
#include <string>
class VirtualDevice;
class SkiaSalGraphicsImpl;
namespace office_graphite {
void initialize(WGPUInstance instance, WGPUDevice device, WGPUQueue queue, size_t cacheBytes);
void shutdown();
bool enabled();
bool isGraphite(const sk_sp<SkImage>& image);
sk_sp<SkSurface> createSurface(const SkImageInfo& info, const SkSurfaceProps* properties);
sk_sp<SkSurface> wrapTexture(WGPUTexture texture);
bool flush(bool wait);
bool healthy();
/** Cause of the first failure that made the backend unusable; empty while it is healthy. */
std::string failure();
// Records a device-lost notification delivered through the WebGPU device descriptor callback.
void deviceLost(WGPUDeviceLostReason reason, const char* message);
// Retains the most recent uncaptured WebGPU error as context for a later device-lost notification.
void deviceError(const char* message);
// Called only on the VCL thread; detaches pending canvases from a failed recorder.
void retireLostDevice();
sk_sp<SkImage> rasterImage(sk_sp<SkImage> image);
bool readSurface(sk_sp<SkSurface> surface, SkBitmap& bitmap);
void observeSnapshot(const SkSurface* surface);
void report();
// A binding changes only this device; intermediate document devices keep their own targets.
class Target {
    SkiaSalGraphicsImpl* implementation = nullptr;
public:
    Target(VirtualDevice& device, WGPUTexture texture);
    ~Target();
    Target(const Target&) = delete;
    Target& operator=(const Target&) = delete;
    void finish();
};
}
