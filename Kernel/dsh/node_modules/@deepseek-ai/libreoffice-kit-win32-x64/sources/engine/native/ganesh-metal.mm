// Metal blits reuse the device that owns LibreOffice's Ganesh texture.
#include <skia/dsh_ganesh.hxx>
#include <ganesh/GrBackendSurface.h>
#include <ganesh/SkSurfaceGanesh.h>
#include <ganesh/mtl/GrMtlBackendSurface.h>
#import <Metal/Metal.h>
#import <IOSurface/IOSurface.h>
#include <stdexcept>

namespace office_ganesh {
void* producerDevice() { return nullptr; }
void probeSurface(const sk_sp<SkSurface>& surface)
{
    @autoreleasepool {
        NSDictionary* properties = @{ (__bridge NSString*)kIOSurfaceWidth: @1,
            (__bridge NSString*)kIOSurfaceHeight: @1, (__bridge NSString*)kIOSurfaceBytesPerElement: @4,
            (__bridge NSString*)kIOSurfacePixelFormat: @0x42475241 };
        IOSurfaceRef target = IOSurfaceCreate((__bridge CFDictionaryRef)properties);
        if (!target) throw std::runtime_error("Ganesh cannot allocate a probe IOSurface");
        try { copySurface(surface, { target, nullptr, 1, 1 }); }
        catch (...) { CFRelease(target); throw; }
        CFRelease(target);
    }
}
void copySurface(const sk_sp<SkSurface>& surface, const Target& target)
{
    @autoreleasepool {
        GrMtlTextureInfo info;
        const auto backend = SkSurfaces::GetBackendTexture(surface.get(), SkSurfaces::BackendHandleAccess::kFlushRead);
        if (!GrBackendTextures::GetMtlTextureInfo(backend, &info))
            throw std::runtime_error("Ganesh output is not a Metal texture");
        id<MTLTexture> source = (__bridge id<MTLTexture>)info.fTexture.get();
        MTLTextureDescriptor* description = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
            width:target.width height:target.height mipmapped:NO];
        description.usage = MTLTextureUsageShaderRead | MTLTextureUsageRenderTarget;
        id<MTLTexture> destination = [source.device newTextureWithDescriptor:description
            iosurface:static_cast<IOSurfaceRef>(target.native) plane:0];
        id<MTLCommandQueue> queue = [source.device newCommandQueue];
        if (!destination || !queue) {
            [destination release]; [queue release];
            throw std::runtime_error("Ganesh could not import the shared IOSurface");
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        id<MTLBlitCommandEncoder> encoder = [command blitCommandEncoder];
        if (!command || !encoder) {
            [destination release]; [queue release];
            throw std::runtime_error("Ganesh could not create a Metal copy command");
        }
        [encoder copyFromTexture:source sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0, 0, 0)
            sourceSize:MTLSizeMake(target.width, target.height, 1) toTexture:destination
            destinationSlice:0 destinationLevel:0 destinationOrigin:MTLOriginMake(0, 0, 0)];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        const bool completed = command.status == MTLCommandBufferStatusCompleted;
        [destination release]; [queue release];
        if (!completed) throw std::runtime_error("Ganesh Metal shared-image copy failed");
    }
}
}
