// GPU-only copies from LibreOffice's existing Ganesh context to acquired display images.
#pragma once
#include <SkSurface.h>
class VirtualDevice;
namespace office_ganesh {
// Borrowed handles remain owned by the acquired frame-forwarding surface.
struct Target {
    void* native;
    void* sharedHandle;
    int width;
    int height;
};
// Count final-surface readbacks separately from internal document bitmap operations.
class OutputScope {
public:
    explicit OutputScope(VirtualDevice& device);
    ~OutputScope();
    OutputScope(const OutputScope&) = delete;
    OutputScope& operator=(const OutputScope&) = delete;
};
void observeReadback(const SkSurface* surface);
void observeBitmapExtraction(const SkSurface* surface);
bool initialized();
bool healthy();
// Validate GPU drawing and shared-image import before loading a document.
void initialize();
// Borrow the matching D3D11 device on Windows; IOSurface producers need no device.
void* producerDevice();
// Exercise platform import/copy using a temporary shared image.
void probeSurface(const sk_sp<SkSurface>& surface);
// Returns only after the GPU copy completes; never reads output pixels on the CPU.
void copy(VirtualDevice& device, const Target& target);
void copySurface(const sk_sp<SkSurface>& surface, const Target& target);
}
