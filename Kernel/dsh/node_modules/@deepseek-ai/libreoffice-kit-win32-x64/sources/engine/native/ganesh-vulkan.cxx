// Import the display pool's D3D11 image into LibreOffice's existing Vulkan device.
#ifndef NOMINMAX
#define NOMINMAX
#endif
#define VK_USE_PLATFORM_WIN32_KHR
#include <skia/dsh_ganesh.hxx>
#include <ganesh/GrBackendSurface.h>
#include <ganesh/SkSurfaceGanesh.h>
#include <ganesh/vk/GrVkBackendSurface.h>
#include <ganesh/vk/GrVkTypes.h>
#include <tools/window/VulkanWindowContext.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <cstring>
#include <stdexcept>
#include <string>

namespace office_ganesh {
namespace {
Microsoft::WRL::ComPtr<ID3D11Device> producer;
const skgpu::VulkanBackendContext& backendContext()
{
    const auto* context = skwindow::internal::VulkanWindowContext::getSharedImageBackendContext();
    if (!context) throw std::runtime_error("Ganesh Vulkan requires external-memory and Win32 keyed-mutex support");
    return *context;
}
VkPhysicalDeviceIDProperties deviceIdentity(const skgpu::VulkanBackendContext& context)
{
    auto getProperties = reinterpret_cast<PFN_vkGetPhysicalDeviceProperties2>(
        context.fGetProc("vkGetPhysicalDeviceProperties2", context.fInstance, VK_NULL_HANDLE));
    if (!getProperties) throw std::runtime_error("Ganesh Vulkan device identity query is unavailable");
    VkPhysicalDeviceIDProperties id{ VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ID_PROPERTIES };
    VkPhysicalDeviceProperties2 properties{ VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, &id };
    getProperties(context.fPhysicalDevice, &properties);
    if (!id.deviceLUIDValid) throw std::runtime_error("Ganesh Vulkan GPU has no Windows adapter identity");
    return id;
}
void require(VkResult result, const char* operation)
{
    if (result != VK_SUCCESS)
        throw std::runtime_error(std::string("Ganesh Vulkan ") + operation + " failed (" + std::to_string(result) + ")");
}

// Every import is released before its display lease ends. The worker's operation
// deadline owns cancellation while the driver completes submitted GPU commands.
class Copy {
    const skgpu::VulkanBackendContext& context;
#define DSH_VK_PROC(name) PFN_vk##name name = reinterpret_cast<PFN_vk##name>(context.fGetProc("vk" #name, context.fInstance, context.fDevice));
    DSH_VK_PROC(CreateImage)
    DSH_VK_PROC(DestroyImage)
    DSH_VK_PROC(GetImageMemoryRequirements)
    DSH_VK_PROC(GetMemoryWin32HandlePropertiesKHR)
    DSH_VK_PROC(AllocateMemory)
    DSH_VK_PROC(FreeMemory)
    DSH_VK_PROC(BindImageMemory)
    DSH_VK_PROC(CreateCommandPool)
    DSH_VK_PROC(DestroyCommandPool)
    DSH_VK_PROC(AllocateCommandBuffers)
    DSH_VK_PROC(BeginCommandBuffer)
    DSH_VK_PROC(EndCommandBuffer)
    DSH_VK_PROC(CmdPipelineBarrier)
    DSH_VK_PROC(CmdCopyImage)
    DSH_VK_PROC(CreateFence)
    DSH_VK_PROC(DestroyFence)
    DSH_VK_PROC(QueueSubmit)
    DSH_VK_PROC(WaitForFences)
#undef DSH_VK_PROC
    VkImage image = VK_NULL_HANDLE;
    VkDeviceMemory memory = VK_NULL_HANDLE;
    VkCommandPool pool = VK_NULL_HANDLE;
    VkFence fence = VK_NULL_HANDLE;
public:
    explicit Copy(const skgpu::VulkanBackendContext& value) : context(value)
    {
        if (!GetMemoryWin32HandlePropertiesKHR)
            throw std::runtime_error("Ganesh Vulkan device cannot import D3D11 shared images");
    }
    ~Copy()
    {
        if (fence) DestroyFence(context.fDevice, fence, nullptr);
        if (pool) DestroyCommandPool(context.fDevice, pool, nullptr);
        if (image) DestroyImage(context.fDevice, image, nullptr);
        if (memory) FreeMemory(context.fDevice, memory, nullptr);
    }
    void run(const GrVkImageInfo& source, const Target& target)
    {
        constexpr auto handleType = VK_EXTERNAL_MEMORY_HANDLE_TYPE_D3D11_TEXTURE_BIT;
        const auto id = deviceIdentity(context);
        Microsoft::WRL::ComPtr<ID3D11Device> device;
        static_cast<ID3D11Texture2D*>(target.native)->GetDevice(&device);
        Microsoft::WRL::ComPtr<IDXGIDevice> dxgi;
        Microsoft::WRL::ComPtr<IDXGIAdapter> adapter;
        DXGI_ADAPTER_DESC description{};
        if (!id.deviceLUIDValid || FAILED(device.As(&dxgi)) || FAILED(dxgi->GetAdapter(&adapter))
            || FAILED(adapter->GetDesc(&description)) || std::memcmp(id.deviceLUID, &description.AdapterLuid, VK_LUID_SIZE) != 0)
            throw std::runtime_error("Ganesh Vulkan and the shared-image pool must use the same GPU");

        VkExternalMemoryImageCreateInfo external{ VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO };
        external.handleTypes = handleType;
        VkImageCreateInfo create{ VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, &external };
        create.imageType = VK_IMAGE_TYPE_2D;
        create.format = VK_FORMAT_B8G8R8A8_UNORM;
        create.extent = { static_cast<uint32_t>(target.width), static_cast<uint32_t>(target.height), 1 };
        create.mipLevels = 1; create.arrayLayers = 1; create.samples = VK_SAMPLE_COUNT_1_BIT;
        create.tiling = VK_IMAGE_TILING_OPTIMAL;
        create.usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
        create.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
        require(CreateImage(context.fDevice, &create, nullptr, &image), "create shared image");
        VkMemoryRequirements requirements{};
        GetImageMemoryRequirements(context.fDevice, image, &requirements);
        VkMemoryWin32HandlePropertiesKHR handles{ VK_STRUCTURE_TYPE_MEMORY_WIN32_HANDLE_PROPERTIES_KHR };
        require(GetMemoryWin32HandlePropertiesKHR(context.fDevice, handleType, target.sharedHandle, &handles), "query shared memory");
        const uint32_t types = requirements.memoryTypeBits & handles.memoryTypeBits;
        if (!types) throw std::runtime_error("Ganesh Vulkan shared image has no compatible memory type");
        uint32_t type = 0;
        while (!(types & (1u << type))) ++type;
        VkMemoryDedicatedAllocateInfo dedicated{ VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO };
        dedicated.image = image;
        VkImportMemoryWin32HandleInfoKHR imported{ VK_STRUCTURE_TYPE_IMPORT_MEMORY_WIN32_HANDLE_INFO_KHR, &dedicated };
        imported.handleType = handleType; imported.handle = target.sharedHandle;
        VkMemoryAllocateInfo allocation{ VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, &imported };
        allocation.allocationSize = requirements.size; allocation.memoryTypeIndex = type;
        require(AllocateMemory(context.fDevice, &allocation, nullptr, &memory), "import shared memory");
        require(BindImageMemory(context.fDevice, image, memory, 0), "bind shared image");

        VkCommandPoolCreateInfo poolInfo{ VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO };
        poolInfo.flags = VK_COMMAND_POOL_CREATE_TRANSIENT_BIT;
        poolInfo.queueFamilyIndex = context.fGraphicsQueueIndex;
        require(CreateCommandPool(context.fDevice, &poolInfo, nullptr, &pool), "create copy command pool");
        VkCommandBufferAllocateInfo commands{ VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO };
        commands.commandPool = pool; commands.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY; commands.commandBufferCount = 1;
        VkCommandBuffer command;
        require(AllocateCommandBuffers(context.fDevice, &commands, &command), "allocate copy command");
        VkCommandBufferBeginInfo begin{ VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO };
        begin.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
        require(BeginCommandBuffer(command, &begin), "begin copy command");
        const VkImageSubresourceRange range{ VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 };
        VkImageMemoryBarrier barriers[2]{};
        for (auto& barrier : barriers) { barrier.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER; barrier.subresourceRange = range; }
        barriers[0].srcAccessMask = VK_ACCESS_MEMORY_WRITE_BIT;
        barriers[0].dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT;
        barriers[0].oldLayout = source.fImageLayout; barriers[0].newLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
        barriers[0].srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED; barriers[0].dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
        barriers[0].image = source.fImage;
        barriers[1].dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        barriers[1].oldLayout = VK_IMAGE_LAYOUT_UNDEFINED; barriers[1].newLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        barriers[1].srcQueueFamilyIndex = VK_QUEUE_FAMILY_EXTERNAL; barriers[1].dstQueueFamilyIndex = context.fGraphicsQueueIndex;
        barriers[1].image = image;
        CmdPipelineBarrier(command, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT, 0,
            0, nullptr, 0, nullptr, 2, barriers);
        VkImageCopy region{};
        region.srcSubresource = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1 };
        region.dstSubresource = region.srcSubresource; region.extent = create.extent;
        CmdCopyImage(command, source.fImage, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
            image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &region);
        barriers[0].srcAccessMask = VK_ACCESS_TRANSFER_READ_BIT; barriers[0].dstAccessMask = VK_ACCESS_MEMORY_READ_BIT | VK_ACCESS_MEMORY_WRITE_BIT;
        barriers[0].oldLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL; barriers[0].newLayout = source.fImageLayout;
        barriers[1].srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT; barriers[1].dstAccessMask = 0;
        barriers[1].oldLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL; barriers[1].newLayout = VK_IMAGE_LAYOUT_GENERAL;
        barriers[1].srcQueueFamilyIndex = context.fGraphicsQueueIndex; barriers[1].dstQueueFamilyIndex = VK_QUEUE_FAMILY_EXTERNAL;
        CmdPipelineBarrier(command, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, 0,
            0, nullptr, 0, nullptr, 2, barriers);
        require(EndCommandBuffer(command), "end copy command");
        VkFenceCreateInfo fenceInfo{ VK_STRUCTURE_TYPE_FENCE_CREATE_INFO };
        require(CreateFence(context.fDevice, &fenceInfo, nullptr, &fence), "create copy fence");
        const uint64_t key = 0;
        const uint32_t timeoutMs = 1000;
        VkWin32KeyedMutexAcquireReleaseInfoKHR mutex{ VK_STRUCTURE_TYPE_WIN32_KEYED_MUTEX_ACQUIRE_RELEASE_INFO_KHR };
        mutex.acquireCount = 1; mutex.pAcquireSyncs = &memory; mutex.pAcquireKeys = &key; mutex.pAcquireTimeouts = &timeoutMs;
        mutex.releaseCount = 1; mutex.pReleaseSyncs = &memory; mutex.pReleaseKeys = &key;
        VkSubmitInfo submit{ VK_STRUCTURE_TYPE_SUBMIT_INFO, &mutex };
        submit.commandBufferCount = 1; submit.pCommandBuffers = &command;
        require(QueueSubmit(context.fQueue, 1, &submit, fence), "submit shared-image copy");
        require(WaitForFences(context.fDevice, 1, &fence, VK_TRUE, UINT64_MAX), "complete shared-image copy");
    }
};
}
void* producerDevice()
{
    if (producer) return producer.Get();
    const auto id = deviceIdentity(backendContext());
    Microsoft::WRL::ComPtr<IDXGIFactory1> factory;
    if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory))))
        throw std::runtime_error("Ganesh cannot enumerate Windows graphics adapters");
    for (UINT index = 0;; ++index) {
        Microsoft::WRL::ComPtr<IDXGIAdapter1> adapter;
        const auto result = factory->EnumAdapters1(index, &adapter);
        if (result == DXGI_ERROR_NOT_FOUND) break;
        DXGI_ADAPTER_DESC1 description{};
        if (FAILED(result) || FAILED(adapter->GetDesc1(&description)))
            throw std::runtime_error("Ganesh cannot inspect a Windows graphics adapter");
        if (std::memcmp(id.deviceLUID, &description.AdapterLuid, VK_LUID_SIZE) != 0) continue;
        if (FAILED(D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION, &producer, nullptr, nullptr)))
            throw std::runtime_error("Ganesh cannot create the matching D3D11 shared-image device");
        return producer.Get();
    }
    throw std::runtime_error("Ganesh Vulkan GPU has no matching D3D11 adapter");
}
void probeSurface(const sk_sp<SkSurface>& surface)
{
    auto* device = static_cast<ID3D11Device*>(producerDevice());
    D3D11_TEXTURE2D_DESC description{};
    description.Width = 1; description.Height = 1; description.MipLevels = 1; description.ArraySize = 1;
    description.Format = DXGI_FORMAT_B8G8R8A8_UNORM; description.SampleDesc.Count = 1;
    description.Usage = D3D11_USAGE_DEFAULT;
    description.BindFlags = D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET;
    description.MiscFlags = D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX;
    Microsoft::WRL::ComPtr<ID3D11Texture2D> texture;
    Microsoft::WRL::ComPtr<IDXGIResource1> resource;
    HANDLE handle = nullptr;
    if (FAILED(device->CreateTexture2D(&description, nullptr, &texture)) || FAILED(texture.As(&resource))
        || FAILED(resource->CreateSharedHandle(nullptr, DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE, nullptr, &handle)))
        throw std::runtime_error("Ganesh cannot allocate a shared D3D11 probe image");
    try { copySurface(surface, { texture.Get(), handle, 1, 1 }); }
    catch (...) { CloseHandle(handle); throw; }
    CloseHandle(handle);
}
void copySurface(const sk_sp<SkSurface>& surface, const Target& target)
{
    const auto& context = backendContext();
    if (!target.sharedHandle) throw std::runtime_error("Ganesh Vulkan requires an acquired shared NT handle");
    const auto backend = SkSurfaces::GetBackendTexture(surface.get(), SkSurfaces::BackendHandleAccess::kFlushRead);
    GrVkImageInfo image;
    if (!GrBackendTextures::GetVkImageInfo(backend, &image) || image.fFormat != VK_FORMAT_B8G8R8A8_UNORM)
        throw std::runtime_error("Ganesh output is not a BGRA Vulkan texture");
    Copy(context).run(image, target);
}
}
