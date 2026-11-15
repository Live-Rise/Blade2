// Preserve VCL drawing and scaling while copying its final image entirely on the GPU.
#include <skia/dsh_ganesh.hxx>
#include <skia/gdiimpl.hxx>
#include <skia/utils.hxx>
#include <salgdi.hxx>
#include <vcl/virdev.hxx>
#include <SkCanvas.h>
#include <SkImage.h>
#include <SkPaint.h>
#include <vcl/skia/SkiaHelper.hxx>
#include <ganesh/GrDirectContext.h>
#include <ganesh/SkSurfaceGanesh.h>
#include <stdexcept>
#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace office_ganesh {
namespace {
const SkSurface* outputTarget = nullptr;
GrDirectContext* drawingContext = nullptr;
size_t gpuCopies = 0, outputReadbacks = 0, outputBitmapExtractions = 0;
sk_sp<SkSurface> deviceSurface(VirtualDevice& device)
{
    auto* graphics = device.GetGraphics();
    auto* implementation = graphics ? dynamic_cast<SkiaSalGraphicsImpl*>(graphics->GetImpl()) : nullptr;
    if (!implementation) throw std::runtime_error("Ganesh output requires a Skia device");
    return implementation->ganeshOutputSurface();
}
}
OutputScope::OutputScope(VirtualDevice& device)
{
    if (outputTarget) throw std::runtime_error("Nested Ganesh output drawing");
    outputTarget = deviceSurface(device).get();
}
OutputScope::~OutputScope()
{
    outputTarget = nullptr;
    const char* diagnostics = std::getenv("OFFICE_GRAPHITE_DIAGNOSTICS");
    if (diagnostics && std::strcmp(diagnostics, "1") == 0)
        std::fprintf(stderr, "officeGanesh gpuCopies=%zu outputReadbacks=%zu outputBitmapExtractions=%zu\n", gpuCopies, outputReadbacks, outputBitmapExtractions);
}
void observeReadback(const SkSurface* surface) { if (surface == outputTarget) ++outputReadbacks; }
void observeBitmapExtraction(const SkSurface* surface) { if (surface == outputTarget) ++outputBitmapExtractions; }
bool initialized() { return drawingContext != nullptr; }
bool healthy()
{
    return drawingContext && !drawingContext->abandoned()
        && SkiaHelper::renderMethodToUse() != SkiaHelper::RenderRaster;
}
void initialize()
{
    auto* context = SkiaHelper::getSharedGrDirectContext();
    if (!context || context->abandoned()) throw std::runtime_error("Ganesh context is unavailable");
    auto surface = SkSurfaces::RenderTarget(context, skgpu::Budgeted::kYes,
        SkImageInfo::Make(1, 1, kBGRA_8888_SkColorType, kPremul_SkAlphaType),
        0, kTopLeft_GrSurfaceOrigin, SkiaHelper::surfaceProps());
    if (!surface) throw std::runtime_error("Ganesh cannot allocate a GPU output surface");
    surface->getCanvas()->clear(SK_ColorBLACK);
    context->flushAndSubmit(surface.get(), GrSyncCpu::kYes);
    probeSurface(surface);
    drawingContext = context;
}
void copy(VirtualDevice& device, const Target& target)
{
    if (!target.native || target.width <= 0 || target.height <= 0)
        throw std::runtime_error("Ganesh output requires an acquired native image");
    auto source = deviceSurface(device);
    auto* context = GrAsDirectContext(source->recordingContext());
    if (!context || context->abandoned())
        throw std::runtime_error("Ganesh output context is unavailable");
    // VCL WITHOUT_ALPHA readback exposes opaque pixels and applies device scaling.
    // Perform the same normalization in a GPU surface before the native copy.
    auto output = SkSurfaces::RenderTarget(context, skgpu::Budgeted::kYes,
        SkImageInfo::Make(target.width, target.height, kBGRA_8888_SkColorType, kPremul_SkAlphaType),
        0, kTopLeft_GrSurfaceOrigin, SkiaHelper::surfaceProps());
    auto image = source->makeImageSnapshot();
    if (!output || !image || !image->isTextureBacked())
        throw std::runtime_error("Ganesh output requires a GPU-backed final image");
    SkPaint paint;
    paint.setBlendMode(SkBlendMode::kSrc);
    const Size sourceSize(image->width(), image->height()), targetSize(target.width, target.height);
    const auto sampling = sourceSize == targetSize ? SkSamplingOptions()
        : SkiaHelper::makeSamplingOptions(SkiaHelper::goodScalingQuality(true), sourceSize, targetSize, 1);
    output->getCanvas()->drawImageRect(image, SkRect::MakeWH(target.width, target.height), sampling, &paint);
    output->getCanvas()->drawColor(SK_ColorBLACK, SkBlendMode::kDstATop);
    context->flushAndSubmit(output.get(), GrSyncCpu::kYes);
    if (context->abandoned()) throw std::runtime_error("Ganesh device lost before shared-image copy");
    copySurface(output, target);
    ++gpuCopies;
}
}
