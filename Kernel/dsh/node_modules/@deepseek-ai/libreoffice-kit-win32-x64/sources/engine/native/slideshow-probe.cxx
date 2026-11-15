// Offscreen presentation views share the native animation engine with desktop
// Impress. The caller drives frames on the LibreOffice initialization thread.
#include <sal/config.h>
#include <sal/log.hxx>
#include <LibreOfficeKit/LibreOfficeKit.h>
#include "lib/init.hxx"
#include <comphelper/processfactory.hxx>
#include <comphelper/scopeguard.hxx>
#include <cppuhelper/implbase.hxx>
#include <com/sun/star/animations/XAnimationNodeSupplier.hpp>
#include <com/sun/star/awt/XWindow.hpp>
#include <com/sun/star/beans/XPropertySet.hpp>
#include <com/sun/star/drawing/XDrawPagesSupplier.hpp>
#include <com/sun/star/lang/XMultiComponentFactory.hpp>
#include <com/sun/star/presentation/XSlideShow.hpp>
#include <com/sun/star/presentation/XSlideShowListener.hpp>
#include <com/sun/star/presentation/XSlideShowView.hpp>
#include <com/sun/star/rendering/XSpriteCanvas.hpp>
#include <com/sun/star/uno/XComponentContext.hpp>
#include <vcl/BitmapReadAccess.hxx>
#include <vcl/svapp.hxx>
#include <vcl/virdev.hxx>
#ifdef SK_GRAPHITE
#include <skia/dsh_graphite.hxx>
#include <skia/dsh_ganesh.hxx>
#endif
#include <algorithm>
#include <chrono>
#include <memory>
#include <stdexcept>
#include <thread>

namespace {
using namespace css;

class OffscreenView final : public cppu::WeakImplHelper<presentation::XSlideShowView>
{
    uno::Reference<rendering::XSpriteCanvas> mCanvas;
    sal_Int32 mWidth, mHeight;
public:
    OffscreenView(uno::Reference<rendering::XSpriteCanvas> canvas, sal_Int32 width, sal_Int32 height)
        : mCanvas(std::move(canvas)), mWidth(width), mHeight(height) {}
    uno::Reference<rendering::XSpriteCanvas> SAL_CALL getCanvas() override { return mCanvas; }
    void SAL_CALL clear() override { mCanvas->clear(); }
    geometry::AffineMatrix2D SAL_CALL getTransformation() override { return { double(mWidth), 0, 0, 0, double(mHeight), 0 }; }
    geometry::IntegerSize2D SAL_CALL getTranslationOffset() override { return { 0, 0 }; }
    awt::Rectangle SAL_CALL getCanvasArea() override { return { 0, 0, mWidth, mHeight }; }
    void SAL_CALL addTransformationChangedListener(const uno::Reference<util::XModifyListener>&) override {}
    void SAL_CALL removeTransformationChangedListener(const uno::Reference<util::XModifyListener>&) override {}
    void SAL_CALL addPaintListener(const uno::Reference<awt::XPaintListener>&) override {}
    void SAL_CALL removePaintListener(const uno::Reference<awt::XPaintListener>&) override {}
    void SAL_CALL addMouseListener(const uno::Reference<awt::XMouseListener>&) override {}
    void SAL_CALL removeMouseListener(const uno::Reference<awt::XMouseListener>&) override {}
    void SAL_CALL addMouseMotionListener(const uno::Reference<awt::XMouseMotionListener>&) override {}
    void SAL_CALL removeMouseMotionListener(const uno::Reference<awt::XMouseMotionListener>&) override {}
    void SAL_CALL setMouseCursor(sal_Int16) override {}
};

class ShowEvents final : public cppu::WeakImplHelper<presentation::XSlideShowListener>
{
public:
    int delta = 0;
    void SAL_CALL disposing(const lang::EventObject&) override {}
    void SAL_CALL beginEvent(const uno::Reference<animations::XAnimationNode>&) override {}
    void SAL_CALL endEvent(const uno::Reference<animations::XAnimationNode>&) override {}
    void SAL_CALL repeat(const uno::Reference<animations::XAnimationNode>&, sal_Int32) override {}
    void SAL_CALL paused() override {}
    void SAL_CALL resumed() override {}
    void SAL_CALL slideTransitionStarted() override {}
    void SAL_CALL slideTransitionEnded() override {}
    void SAL_CALL slideAnimationsEnded() override {}
    void SAL_CALL slideEnded(sal_Bool reverse) override { delta = reverse ? -1 : 1; }
    void SAL_CALL hyperLinkClicked(const OUString&) override {}
};

// Components retain rendering resources; dispose while their output device and
// document still exist, including on partial initialization failure.
struct ComponentOwner {
    uno::Reference<lang::XComponent> component;
    void reset()
    {
        if (!component.is()) return;
        try { component->dispose(); }
        catch (const uno::Exception& error) { SAL_WARN("desktop.lib", "Slideshow disposal failed: " << error.Message); }
        component.clear();
    }
    ~ComponentOwner() { reset(); }
};

struct Player {
    ScopedVclPtrInstance<VirtualDevice> device;
    uno::Reference<rendering::XSpriteCanvas> canvas;
    uno::Reference<beans::XPropertySet> canvasProperties;
    ComponentOwner canvasOwner;
    uno::Reference<presentation::XSlideShow> show;
    ComponentOwner showOwner;
    uno::Reference<presentation::XSlideShowView> view;
    uno::Reference<drawing::XDrawPagesSupplier> pages;
    rtl::Reference<ShowEvents> events = new ShowEvents;
    int width, height, part = 0;
    bool ended = false;
    double nextUpdate = -1;

    Player(LibreOfficeKitDocument* document, int w, int h) : width(w), height(h)
    {
        const auto context = comphelper::getProcessComponentContext();
        show.set(context->getServiceManager()->createInstanceWithContext(u"com.sun.star.presentation.SlideShow"_ustr, context), uno::UNO_QUERY_THROW);
        showOwner.component.set(show, uno::UNO_QUERY_THROW);
        pages.set(static_cast<desktop::LibLODocument_Impl*>(document)->mxComponent, uno::UNO_QUERY_THROW);
        show->addSlideShowListener(events);
        resize(w, h);
    }
    void resize(int w, int h)
    {
        if (view.is()) { show->removeView(view); view.clear(); }
        canvasOwner.reset(); canvas.clear(); canvasProperties.clear();
        width = w; height = h;
        if (!device->SetOutputSizePixel(Size(width, height))) throw std::runtime_error("Cannot allocate slideshow frame");
        device->SetBackground(Wallpaper(COL_WHITE)); device->Erase();
        const auto context = comphelper::getProcessComponentContext();
        const uno::Sequence<uno::Any> arguments {
            uno::Any(reinterpret_cast<sal_Int64>(static_cast<OutputDevice*>(device.get()))),
            uno::Any(awt::Rectangle(0, 0, width, height)), uno::Any(false), uno::Any(uno::Reference<awt::XWindow>()) };
        canvas.set(context->getServiceManager()->createInstanceWithArgumentsAndContext(
            u"com.sun.star.rendering.SpriteCanvas.VCL"_ustr, arguments, context), uno::UNO_QUERY_THROW);
        canvasOwner.component.set(canvas, uno::UNO_QUERY_THROW);
        canvasProperties.set(canvas, uno::UNO_QUERY_THROW);
        canvasProperties->setPropertyValue(u"LOKDeferredUpdate"_ustr, uno::Any(true));
        view = new OffscreenView(canvas, width, height);
        if (!show->addView(view)) throw std::runtime_error("Cannot attach slideshow view");
        canvas->updateScreen(true);
    }
    ~Player() { showOwner.reset(); canvasOwner.reset(); }
    void display(int index)
    {
        if (index < 0 || index >= pages->getDrawPages()->getCount()) { ended = index >= pages->getDrawPages()->getCount(); return; }
        part = index; ended = false; events->delta = 0;
        uno::Reference<drawing::XDrawPage> page(pages->getDrawPages()->getByIndex(part), uno::UNO_QUERY_THROW);
        uno::Reference<animations::XAnimationNodeSupplier> animation(page, uno::UNO_QUERY_THROW);
        show->displaySlide(page, pages, animation->getAnimationNode(), {});
    }
    bool advance(bool force = false)
    {
        double timeout = 0;
        bool active = show->update(timeout);
        if (events->delta) { const int delta = events->delta; events->delta = 0; display(part + delta); active = show->update(timeout); }
        nextUpdate = active ? std::max(0.0, timeout) : -1;
        Application::Reschedule(true);
        if (force) canvas->updateScreen(true);
        bool pending = false;
        canvasProperties->getPropertyValue(u"LOKUpdatePending"_ustr) >>= pending;
        return pending;
    }
    void drawPrepared()
    {
        canvasProperties->setPropertyValue(u"LOKDeferredUpdate"_ustr, uno::Any(false));
        comphelper::ScopeGuard defer([&] {
            canvasProperties->setPropertyValue(u"LOKDeferredUpdate"_ustr, uno::Any(true));
        });
        if (!canvas->updateScreen(true)) throw std::runtime_error("Offscreen canvas refused to refresh");
    }
#ifdef SK_GRAPHITE
    void drawGanesh(const office_ganesh::Target& target)
    {
        office_ganesh::OutputScope output(*device);
        drawPrepared();
        office_ganesh::copy(*device, target);
    }
    void drawGraphite(WGPUTexture texture)
    {
        office_graphite::Target target(*device, texture);
        device->Erase();
        drawPrepared();
        target.finish();
    }
    void graphite(WGPUTexture texture)
    {
        advance();
        drawGraphite(texture);
    }
#endif
    void drawPixels(unsigned char* pixels, bool bgra = false)
    {
        drawPrepared();
        const Bitmap bitmap = device->GetBitmap(Point(0, 0), Size(width, height));
        BitmapScopedReadAccess access(bitmap);
        if (!access) throw std::runtime_error("Offscreen canvas has no readable bitmap");
        const auto format = access->GetScanlineFormat();
        const bool bgr = format == ScanlineFormat::N32BitTcBgrx || format == ScanlineFormat::N24BitTcBgr;
        const bool rgb = format == ScanlineFormat::N32BitTcRgbx || format == ScanlineFormat::N24BitTcRgb;
        if (bgr || rgb) {
            const int channels = format == ScanlineFormat::N24BitTcBgr || format == ScanlineFormat::N24BitTcRgb ? 3 : 4;
            const int first = bgra == bgr ? 0 : 2, last = 2 - first;
            for (int y = 0; y < height; ++y) {
                const auto* row = access->GetScanline(y);
                for (int x = 0; x < width; ++x, row += channels) {
                    *pixels++ = row[first]; *pixels++ = row[1]; *pixels++ = row[last]; *pixels++ = 255;
                }
            }
        } else {
            // Palette and premultiplied formats require VCL's color conversion.
            for (int y = 0; y < height; ++y) for (int x = 0; x < width; ++x) {
                const auto color = access->GetColor(y, x);
                *pixels++ = bgra ? color.GetBlue() : color.GetRed(); *pixels++ = color.GetGreen(); *pixels++ = bgra ? color.GetRed() : color.GetBlue(); *pixels++ = 255;
            }
        }
    }
    void tick(unsigned char* pixels, bool bgra = false)
    {
        advance();
        drawPixels(pixels, bgra);
    }
};
}

extern "C" SAL_DLLPUBLIC_EXPORT void* dsh_native_slideshow_create(LibreOfficeKitDocument* document, int width, int height, int part)
{
    SolarMutexGuard guard;
    if (!document || width < 1 || height < 1 || width > 4096 || height > 4096) throw std::runtime_error("Invalid slideshow size");
    auto player = std::make_unique<Player>(document, width, height);
    player->display(part);
    return player.release();
}
extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_destroy(void* player)
{
    SolarMutexGuard guard;
    delete static_cast<Player*>(player);
}
extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_control(void* pointer, int action, int argument)
{
    SolarMutexGuard guard;
    auto& player = *static_cast<Player*>(pointer);
    switch (action) {
        case 0: player.show->nextEffect(); break;
        case 1: player.show->previousEffect(); break;
        case 2: player.show->pause(argument != 0); break;
        case 3: player.display(argument); break;
        default: throw std::runtime_error("Invalid slideshow action");
    }
}
extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_resize(void* pointer, int width, int height)
{
    SolarMutexGuard guard;
    if (width < 1 || height < 1 || width > 4096 || height > 4096) throw std::runtime_error("Invalid slideshow size");
    static_cast<Player*>(pointer)->resize(width, height);
}
extern "C" SAL_DLLPUBLIC_EXPORT int dsh_native_slideshow_frame(void* pointer, unsigned char* pixels, int* part)
{
    SolarMutexGuard guard;
    auto& player = *static_cast<Player*>(pointer);
    player.tick(pixels); *part = player.part;
    return player.ended ? 1 : 0;
}

extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_frame_rate(void* pointer, int framesPerSecond)
{
    beans::PropertyValue property; property.Name = "LOKPreferredFramesPerSecond";
    property.Value <<= static_cast<sal_Int32>(framesPerSecond);
    if (!static_cast<Player*>(pointer)->show->setProperty(property)) throw std::runtime_error("Unsupported native slideshow frame rate");
}

extern "C" SAL_DLLPUBLIC_EXPORT int dsh_native_slideshow_state(void* pointer, int* part)
{
    const auto& player = *static_cast<Player*>(pointer); *part = player.part;
    return player.ended ? 1 : 0;
}

extern "C" SAL_DLLPUBLIC_EXPORT double dsh_native_slideshow_delay(void* pointer)
{
    return static_cast<Player*>(pointer)->nextUpdate;
}

// Prepare retained slideshow state before acquiring a transport surface. A
// pending canvas update survives a missed credit and includes the final frame.
extern "C" SAL_DLLPUBLIC_EXPORT bool dsh_native_slideshow_prepare(void* pointer, bool force)
{
    SolarMutexGuard guard;
    return static_cast<Player*>(pointer)->advance(force);
}

extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_draw_bgra(void* pointer, unsigned char* pixels)
{
    SolarMutexGuard guard;
    static_cast<Player*>(pointer)->drawPixels(pixels, true);
}

extern "C" SAL_DLLPUBLIC_EXPORT int dsh_native_slideshow_frame_bgra(void* pointer, unsigned char* pixels, int* part)
{
    SolarMutexGuard guard;
    auto& player = *static_cast<Player*>(pointer);
    player.tick(pixels, true); *part = player.part;
    return player.ended ? 1 : 0;
}

// The diagnostic uses the production player and its clock: four RGBA frames
// before the click and at 100/500/1200 ms after starting the next effect.
extern "C" SAL_DLLPUBLIC_EXPORT bool dsh_native_probe_slideshow(
    LibreOfficeKitDocument* document, unsigned char* frames, int width, int height)
{
    try {
        std::unique_ptr<void, decltype(&dsh_native_slideshow_destroy)> player(
            dsh_native_slideshow_create(document, width, height, 0), dsh_native_slideshow_destroy);
        int part = 0;
        dsh_native_slideshow_frame(player.get(), frames, &part);
        dsh_native_slideshow_control(player.get(), 0, 0);
        const auto started = std::chrono::steady_clock::now();
        int index = 1;
        for (int milliseconds : {100, 500, 1200}) {
            auto* destination = frames + static_cast<size_t>(index++) * width * height * 4;
            do {
                dsh_native_slideshow_frame(player.get(), destination, &part);
                std::this_thread::sleep_for(std::chrono::milliseconds(1));
            } while (std::chrono::steady_clock::now() - started < std::chrono::milliseconds(milliseconds));
        }
        return true;
    } catch (const uno::Exception& error) { SAL_WARN("desktop.lib", "Offscreen slideshow failed: " << error.Message); }
    catch (const std::exception& error) { SAL_WARN("desktop.lib", "Offscreen slideshow failed: " << error.what()); }
    return false;
}

#ifdef SK_GRAPHITE
extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_draw_ganesh(void* handle, const office_ganesh::Target* target)
{
    SolarMutexGuard guard;
    static_cast<Player*>(handle)->drawGanesh(*target);
}

extern "C" SAL_DLLPUBLIC_EXPORT int dsh_native_slideshow_ganesh(void* handle, const office_ganesh::Target* target, int* part)
{
    SolarMutexGuard guard;
    auto& player = *static_cast<Player*>(handle);
    player.advance();
    player.drawGanesh(*target);
    *part = player.part;
    return player.ended;
}

extern "C" SAL_DLLPUBLIC_EXPORT void dsh_native_slideshow_draw_graphite(void* handle, WGPUTexture texture)
{
    SolarMutexGuard guard;
    static_cast<Player*>(handle)->drawGraphite(texture);
}

extern "C" SAL_DLLPUBLIC_EXPORT int dsh_native_slideshow_graphite(void* handle, WGPUTexture texture, int* part)
{
    SolarMutexGuard guard;
    auto& player = *static_cast<Player*>(handle);
    player.graphite(texture);
    *part = player.part;
    return player.ended;
}
#endif
