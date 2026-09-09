#include "astro/renderer.hpp"
#include "astro/background_calibration.hpp"
#include "astro/image_mips.hpp"
#include "astro/photometry.hpp"
#include <SDL3/SDL_vulkan.h>
#include <algorithm>
#include <array>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <imgui.h>
#include <imgui_impl_vulkan.h>
#include <iostream>
#include <png.h>
#include <stdexcept>

namespace astro {
static void check(VkResult r) {
    if (r != VK_SUCCESS) {
        throw std::runtime_error("Vulkan error " + std::to_string(r));
    }
}

static bool extension(const std::vector<VkExtensionProperties>& p, const char* n) {
    return std::any_of(p.begin(), p.end(), [&](auto& x) {
        return !strcmp(x.extensionName, n);
    });
}

VKAPI_ATTR VkBool32 VKAPI_CALL
Renderer::debug_callback(VkDebugUtilsMessageSeverityFlagBitsEXT severity,
                         VkDebugUtilsMessageTypeFlagsEXT,
                         const VkDebugUtilsMessengerCallbackDataEXT* data,
                         void* user) {
    if (severity >= VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT) {
        ++static_cast<Renderer*>(user)->errors_;
    }
    std::cerr << "[Vulkan validation] " << data->pMessage << '\n';
    return VK_FALSE;
}

uint32_t Renderer::memory_type(uint32_t bits, VkMemoryPropertyFlags flags) const {
    VkPhysicalDeviceMemoryProperties p;
    vkGetPhysicalDeviceMemoryProperties(physical_, &p);
    for (uint32_t i = 0; i < p.memoryTypeCount; i++) {
        if ((bits & (1u << i)) && (p.memoryTypes[i].propertyFlags & flags) == flags) {
            return i;
        }
    }
    throw std::runtime_error("No compatible Vulkan memory type");
}

Renderer::Buffer Renderer::buffer(VkDeviceSize size, VkBufferUsageFlags usage) {
    Buffer b;
    b.size = size;
    VkBufferCreateInfo info{VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO};
    info.size = size;
    info.usage = usage;
    info.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
    check(vkCreateBuffer(device_, &info, nullptr, &b.handle));
    VkMemoryRequirements req;
    vkGetBufferMemoryRequirements(device_, b.handle, &req);
    VkMemoryAllocateInfo alloc{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
    alloc.allocationSize = req.size;
    alloc.memoryTypeIndex =
        memory_type(req.memoryTypeBits,
                    VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
    check(vkAllocateMemory(device_, &alloc, nullptr, &b.memory));
    check(vkBindBufferMemory(device_, b.handle, b.memory, 0));
    check(vkMapMemory(device_, b.memory, 0, VK_WHOLE_SIZE, 0, &b.mapped));
    return b;
}

void Renderer::release(Buffer& b) {
    if (b.memory && b.mapped) {
        vkUnmapMemory(device_, b.memory);
    }
    if (b.handle) {
        vkDestroyBuffer(device_, b.handle, nullptr);
    }
    if (b.memory) {
        vkFreeMemory(device_, b.memory, nullptr);
    }
    b = {};
}

Renderer::Renderer(SDL_Window* w,
                   const std::filesystem::path& shaders,
                   const std::filesystem::path& background,
                   bool validation)
    : window_(w), shaders_(shaders) {
    try {
        uint32_t n = 0;
        const char* const* needed = SDL_Vulkan_GetInstanceExtensions(&n);
        if (!needed) {
            throw std::runtime_error(SDL_GetError());
        }
        std::vector<const char*> extensions(needed, needed + n);
        uint32_t count;
        check(vkEnumerateInstanceExtensionProperties(nullptr, &count, nullptr));
        std::vector<VkExtensionProperties> available(count);
        check(vkEnumerateInstanceExtensionProperties(nullptr, &count, available.data()));
        VkApplicationInfo app{VK_STRUCTURE_TYPE_APPLICATION_INFO};
        app.pApplicationName = "Astra";
        app.applicationVersion = VK_MAKE_VERSION(0, 1, 0);
        app.apiVersion = VK_API_VERSION_1_2;
        VkInstanceCreateInfo info{VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO};
        info.pApplicationInfo = &app;
        if (extension(available, "VK_KHR_portability_enumeration")) {
            extensions.push_back("VK_KHR_portability_enumeration");
            info.flags |= VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR;
        }
        const char* layer = "VK_LAYER_KHRONOS_validation";
        VkDebugUtilsMessengerCreateInfoEXT dbg{
            VK_STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT};
        dbg.messageSeverity = VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT |
                              VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT;
        dbg.messageType = VK_DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT |
                          VK_DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT |
                          VK_DEBUG_UTILS_MESSAGE_TYPE_PERFORMANCE_BIT_EXT;
        dbg.pfnUserCallback = debug_callback;
        dbg.pUserData = this;
        if (validation) {
            uint32_t lc;
            check(vkEnumerateInstanceLayerProperties(&lc, nullptr));
            std::vector<VkLayerProperties> layers(lc);
            check(vkEnumerateInstanceLayerProperties(&lc, layers.data()));
            if (std::none_of(layers.begin(), layers.end(), [&](auto& l) {
                    return !strcmp(l.layerName, layer);
                })) {
                throw std::runtime_error("Validation layer unavailable; check VK_LAYER_PATH");
            }
            extensions.push_back(VK_EXT_DEBUG_UTILS_EXTENSION_NAME);
            info.enabledLayerCount = 1;
            info.ppEnabledLayerNames = &layer;
            info.pNext = &dbg;
        }
        info.enabledExtensionCount = uint32_t(extensions.size());
        info.ppEnabledExtensionNames = extensions.data();
        check(vkCreateInstance(&info, nullptr, &instance_));
        if (validation) {
            auto create = (PFN_vkCreateDebugUtilsMessengerEXT)vkGetInstanceProcAddr(
                instance_, "vkCreateDebugUtilsMessengerEXT");
            check(create(instance_, &dbg, nullptr, &debug_));
        }
        if (!SDL_Vulkan_CreateSurface(window_, instance_, nullptr, &surface_)) {
            throw std::runtime_error(SDL_GetError());
        }
        check(vkEnumeratePhysicalDevices(instance_, &count, nullptr));
        std::vector<VkPhysicalDevice> devices(count);
        check(vkEnumeratePhysicalDevices(instance_, &count, devices.data()));
        for (auto device : devices) {
            VkPhysicalDeviceProperties props;
            vkGetPhysicalDeviceProperties(device, &props);
            if (props.apiVersion < VK_API_VERSION_1_2) {
                continue;
            }
            uint32_t qc;
            vkGetPhysicalDeviceQueueFamilyProperties(device, &qc, nullptr);
            std::vector<VkQueueFamilyProperties> families(qc);
            vkGetPhysicalDeviceQueueFamilyProperties(device, &qc, families.data());
            for (uint32_t i = 0; i < qc; i++) {
                VkBool32 present;
                check(vkGetPhysicalDeviceSurfaceSupportKHR(device, i, surface_, &present));
                if (present && (families[i].queueFlags & VK_QUEUE_GRAPHICS_BIT)) {
                    physical_ = device;
                    family_ = i;
                    gpu_ = props.deviceName;
                    break;
                }
            }
            if (physical_) {
                break;
            }
        }
        if (!physical_) {
            throw std::runtime_error("A Vulkan 1.2 graphics/present device is required");
        }
        check(vkEnumerateDeviceExtensionProperties(physical_, nullptr, &count, nullptr));
        available.resize(count);
        check(vkEnumerateDeviceExtensionProperties(physical_, nullptr, &count, available.data()));
        std::vector<const char*> de{VK_KHR_SWAPCHAIN_EXTENSION_NAME};
        if (extension(available, "VK_KHR_portability_subset")) {
            de.push_back("VK_KHR_portability_subset");
        }
        float priority = 1;
        VkDeviceQueueCreateInfo qi{VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO};
        qi.queueFamilyIndex = family_;
        qi.queueCount = 1;
        qi.pQueuePriorities = &priority;
        VkDeviceCreateInfo di{VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO};
        di.queueCreateInfoCount = 1;
        di.pQueueCreateInfos = &qi;
        di.enabledExtensionCount = uint32_t(de.size());
        di.ppEnabledExtensionNames = de.data();
        VkPhysicalDeviceFeatures supported{}, enabled{};
        vkGetPhysicalDeviceFeatures(physical_, &supported);
        enabled.samplerAnisotropy = supported.samplerAnisotropy;
        di.pEnabledFeatures = &enabled;
        if (enabled.samplerAnisotropy) {
            VkPhysicalDeviceProperties properties;
            vkGetPhysicalDeviceProperties(physical_, &properties);
            background_anisotropy_ = std::min(8.f, properties.limits.maxSamplerAnisotropy);
        }
        check(vkCreateDevice(physical_, &di, nullptr, &device_));
        vkGetDeviceQueue(device_, family_, 0, &queue_);
        VkDescriptorPoolSize pools[] = {{VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 64},
                                        {VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER, 2}};
        VkDescriptorPoolCreateInfo dp{VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO};
        dp.flags = VK_DESCRIPTOR_POOL_CREATE_FREE_DESCRIPTOR_SET_BIT;
        dp.maxSets = 32;
        dp.poolSizeCount = 2;
        dp.pPoolSizes = pools;
        check(vkCreateDescriptorPool(device_, &dp, nullptr, &descriptors_));
        VkDescriptorSetLayoutBinding bind{
            0, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 1, VK_SHADER_STAGE_FRAGMENT_BIT, nullptr};
        VkDescriptorSetLayoutCreateInfo dl{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO};
        dl.bindingCount = 1;
        dl.pBindings = &bind;
        check(vkCreateDescriptorSetLayout(device_, &dl, nullptr, &tone_set_layout_));
        std::array<VkDescriptorSetLayoutBinding, 9> atmosphere_bindings{};
        for (uint32_t i = 0; i < atmosphere_bindings.size(); ++i) {
            atmosphere_bindings[i] = {i,
                                      i == 8 ? VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER
                                             : VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                                      1,
                                      VK_SHADER_STAGE_FRAGMENT_BIT,
                                      nullptr};
        }
        dl.bindingCount = uint32_t(atmosphere_bindings.size());
        dl.pBindings = atmosphere_bindings.data();
        check(vkCreateDescriptorSetLayout(device_, &dl, nullptr, &atmosphere_set_layout_));
        VkSamplerCreateInfo si{VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO};
        si.magFilter = si.minFilter = VK_FILTER_LINEAR;
        si.addressModeU = si.addressModeV = si.addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
        si.maxLod = 0;
        check(vkCreateSampler(device_, &si, nullptr, &sampler_));
        VkFormatProperties fp;
        vkGetPhysicalDeviceFormatProperties(physical_, VK_FORMAT_R32G32B32A32_SFLOAT, &fp);
        manual_atmosphere_filtering_ =
            !(fp.optimalTilingFeatures & VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT) ||
            std::getenv("ASTRA_ATMOSPHERE_MANUAL_FILTER");
        vkGetPhysicalDeviceFormatProperties(physical_, hdr_format_, &fp);
        auto required = VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BIT |
                        VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND_BIT |
                        VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT;
        if ((fp.optimalTilingFeatures & required) != required) {
            throw std::runtime_error("RGBA32F blending is required for photometric HDR rendering");
        }
        const auto atmosphere_path = background.parent_path().parent_path() / "atmosphere";
        for (unsigned i = 0; i < 2; ++i) {
            atmospheres_[i] = std::make_unique<AtmosphereLut>(
                physical_,
                device_,
                queue_,
                family_,
                shaders_,
                atmosphere_path / (i == 0 ? "clear.bin" : "hazy.bin"),
                i);
        }
        for (auto& f : frames_) {
            f.atmosphere = buffer(80, VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT);
            VkDescriptorSetAllocateInfo allocate{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO};
            allocate.descriptorPool = descriptors_;
            allocate.descriptorSetCount = 1;
            allocate.pSetLayouts = &atmosphere_set_layout_;
            check(vkAllocateDescriptorSets(device_, &allocate, &f.atmosphere_set));
            std::array<VkDescriptorImageInfo, 8> images{};
            std::array<VkWriteDescriptorSet, 9> writes{};
            VkDescriptorBufferInfo uniform{f.atmosphere.handle, 0, 80};
            for (uint32_t i = 0; i < writes.size(); ++i) {
                auto& write = writes[i];
                write.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;
                write.dstSet = f.atmosphere_set;
                write.dstBinding = i;
                write.descriptorCount = 1;
                write.descriptorType = atmosphere_bindings[i].descriptorType;
                if (i == 8) {
                    write.pBufferInfo = &uniform;
                } else {
                    images[i] = atmospheres_[i / 4]->descriptor(i % 4);
                    write.pImageInfo = &images[i];
                }
            }
            vkUpdateDescriptorSets(device_, uint32_t(writes.size()), writes.data(), 0, nullptr);
            VkCommandPoolCreateInfo ci{VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO};
            ci.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
            ci.queueFamilyIndex = family_;
            check(vkCreateCommandPool(device_, &ci, nullptr, &f.pool));
            VkCommandBufferAllocateInfo ai{VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO};
            ai.commandPool = f.pool;
            ai.commandBufferCount = 1;
            ai.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
            check(vkAllocateCommandBuffers(device_, &ai, &f.command));
            VkFenceCreateInfo fi{VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
            fi.flags = VK_FENCE_CREATE_SIGNALED_BIT;
            check(vkCreateFence(device_, &fi, nullptr, &f.fence));
            VkSemaphoreCreateInfo sem{VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
            check(vkCreateSemaphore(device_, &sem, nullptr, &f.acquired));
        }
        load_background(background);
        create_swapchain();
        create_pipelines();
        std::cout << "Vulkan GPU: " << gpu_ << "; HDR format " << hdr_format_ << std::endl;
    } catch (...) {
        cleanup();
        throw;
    }
}

void Renderer::load_background(const std::filesystem::path& path) {
    png_image source{};
    source.version = PNG_IMAGE_VERSION;
    if (!png_image_begin_read_from_file(&source, path.string().c_str())) {
        const std::string error = source.message;
        png_image_free(&source);
        throw std::runtime_error("Cannot read Milky Way texture: " + error);
    }
    if (!source.width || source.width != source.height * 2 || source.width > 16384) {
        png_image_free(&source);
        throw std::runtime_error("Milky Way texture must be a 2:1 all-sky PNG, at most 16384 wide");
    }
    source.format = PNG_FORMAT_RGBA;
    std::vector<unsigned char> pixels(PNG_IMAGE_SIZE(source));
    VkExtent3D size{source.width, source.height, 1};
    if (!png_image_finish_read(&source, nullptr, pixels.data(), 0, nullptr)) {
        const std::string error = source.message;
        png_image_free(&source);
        throw std::runtime_error("Cannot decode Milky Way texture: " + error);
    }
    png_image_free(&source);

    const auto mips = make_srgb_mips(size.width, size.height, pixels);
    std::vector<unsigned char>().swap(pixels);
    // Upload the sharpest level supported by the device. Older Vulkan devices
    // can use the same data pack without requiring a 16K texture allocation.
    VkImageFormatProperties format_properties;
    check(vkGetPhysicalDeviceImageFormatProperties(physical_,
                                                   VK_FORMAT_R8G8B8A8_SRGB,
                                                   VK_IMAGE_TYPE_2D,
                                                   VK_IMAGE_TILING_OPTIMAL,
                                                   VK_IMAGE_USAGE_TRANSFER_DST_BIT |
                                                       VK_IMAGE_USAGE_SAMPLED_BIT,
                                                   0,
                                                   &format_properties));
    uint32_t first_level = 0;
    while (size.width > format_properties.maxExtent.width ||
           size.height > format_properties.maxExtent.height) {
        const auto& level = mips.levels.at(++first_level);
        size = {level.width, level.height, 1};
    }
    const auto level_count = uint32_t(mips.levels.size()) - first_level;
    const auto upload_offset = mips.levels[first_level].offset;
    const auto upload_bytes = mips.pixels.size() - upload_offset;
    std::cout << "Milky Way: " << size.width << 'x' << size.height << "; " << level_count
              << " mip levels; anisotropy " << background_anisotropy_ << 'x' << std::endl;

    VkImageCreateInfo image{VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO};
    image.imageType = VK_IMAGE_TYPE_2D;
    image.format = VK_FORMAT_R8G8B8A8_SRGB;
    image.extent = size;
    image.mipLevels = level_count;
    image.arrayLayers = 1;
    image.samples = VK_SAMPLE_COUNT_1_BIT;
    image.tiling = VK_IMAGE_TILING_OPTIMAL;
    image.usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT;
    image.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
    check(vkCreateImage(device_, &image, nullptr, &background_));
    VkMemoryRequirements requirements;
    vkGetImageMemoryRequirements(device_, background_, &requirements);
    VkMemoryAllocateInfo allocation{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
    allocation.allocationSize = requirements.size;
    allocation.memoryTypeIndex =
        memory_type(requirements.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
    check(vkAllocateMemory(device_, &allocation, nullptr, &background_memory_));
    check(vkBindImageMemory(device_, background_, background_memory_, 0));
    VkImageViewCreateInfo view{VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO};
    view.image = background_;
    view.viewType = VK_IMAGE_VIEW_TYPE_2D;
    view.format = image.format;
    view.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, level_count, 0, 1};
    check(vkCreateImageView(device_, &view, nullptr, &background_view_));
    VkSamplerCreateInfo sampler{VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO};
    sampler.magFilter = sampler.minFilter = VK_FILTER_LINEAR;
    sampler.mipmapMode = VK_SAMPLER_MIPMAP_MODE_LINEAR;
    sampler.maxLod = float(level_count - 1);
    sampler.anisotropyEnable = background_anisotropy_ > 1;
    sampler.maxAnisotropy = background_anisotropy_;
    sampler.addressModeU = VK_SAMPLER_ADDRESS_MODE_REPEAT;
    sampler.addressModeV = sampler.addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
    check(vkCreateSampler(device_, &sampler, nullptr, &background_sampler_));

    auto staging = buffer(upload_bytes, VK_BUFFER_USAGE_TRANSFER_SRC_BIT);
    try {
        memcpy(staging.mapped, mips.pixels.data() + upload_offset, upload_bytes);
        auto& frame = frames_[0];
        VkCommandBufferBeginInfo begin{VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
        begin.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
        check(vkBeginCommandBuffer(frame.command, &begin));
        VkImageMemoryBarrier barrier{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
        barrier.oldLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        barrier.newLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        barrier.dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        barrier.srcQueueFamilyIndex = barrier.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
        barrier.image = background_;
        barrier.subresourceRange = view.subresourceRange;
        vkCmdPipelineBarrier(frame.command,
                             VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             0,
                             0,
                             nullptr,
                             0,
                             nullptr,
                             1,
                             &barrier);
        std::vector<VkBufferImageCopy> copies;
        for (uint32_t level = 0; level < level_count; ++level) {
            const auto& mip = mips.levels[first_level + level];
            VkBufferImageCopy copy{};
            copy.bufferOffset = mip.offset - upload_offset;
            copy.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, level, 0, 1};
            copy.imageExtent = {mip.width, mip.height, 1};
            copies.push_back(copy);
        }
        vkCmdCopyBufferToImage(frame.command,
                               staging.handle,
                               background_,
                               VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                               level_count,
                               copies.data());
        barrier.oldLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        barrier.newLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
        barrier.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        barrier.dstAccessMask = VK_ACCESS_SHADER_READ_BIT;
        vkCmdPipelineBarrier(frame.command,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,
                             0,
                             0,
                             nullptr,
                             0,
                             nullptr,
                             1,
                             &barrier);
        check(vkEndCommandBuffer(frame.command));
        check(vkResetFences(device_, 1, &frame.fence));
        VkSubmitInfo submit{VK_STRUCTURE_TYPE_SUBMIT_INFO};
        submit.commandBufferCount = 1;
        submit.pCommandBuffers = &frame.command;
        check(vkQueueSubmit(queue_, 1, &submit, frame.fence));
        check(vkWaitForFences(device_, 1, &frame.fence, VK_TRUE, UINT64_MAX));
    } catch (...) {
        vkDeviceWaitIdle(device_);
        release(staging);
        throw;
    }
    release(staging);
    VkDescriptorSetAllocateInfo set{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO};
    set.descriptorPool = descriptors_;
    set.descriptorSetCount = 1;
    set.pSetLayouts = &tone_set_layout_;
    check(vkAllocateDescriptorSets(device_, &set, &background_set_));
    VkDescriptorImageInfo sampled{
        background_sampler_, background_view_, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
    VkWriteDescriptorSet write{VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET};
    write.dstSet = background_set_;
    write.descriptorCount = 1;
    write.descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
    write.pImageInfo = &sampled;
    vkUpdateDescriptorSets(device_, 1, &write, 0, nullptr);
}

void Renderer::init_ui() {
    ImGui_ImplVulkan_InitInfo i{};
    i.ApiVersion = VK_API_VERSION_1_2;
    i.Instance = instance_;
    i.PhysicalDevice = physical_;
    i.Device = device_;
    i.QueueFamily = family_;
    i.Queue = queue_;
    i.DescriptorPool = descriptors_;
    i.RenderPass = render_pass_;
    i.MinImageCount = 2;
    i.ImageCount = uint32_t(images_.size());
    i.MSAASamples = VK_SAMPLE_COUNT_1_BIT;
    i.CheckVkResultFn = check;
    if (!ImGui_ImplVulkan_Init(&i)) {
        throw std::runtime_error("ImGui Vulkan init failed");
    }
    imgui_ready_ = true;
    ImGui_ImplVulkan_CreateFontsTexture();
}

void Renderer::create_swapchain() {
    int w, h;
    SDL_GetWindowSizeInPixels(window_, &w, &h);
    if (w <= 0 || h <= 0) {
        return;
    }
    VkSurfaceCapabilitiesKHR cap;
    check(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(physical_, surface_, &cap));
    uint32_t count;
    check(vkGetPhysicalDeviceSurfaceFormatsKHR(physical_, surface_, &count, nullptr));
    std::vector<VkSurfaceFormatKHR> formats(count);
    check(vkGetPhysicalDeviceSurfaceFormatsKHR(physical_, surface_, &count, formats.data()));
    auto chosen = formats.front();
    for (auto f : formats) {
        if (f.format == VK_FORMAT_B8G8R8A8_UNORM &&
            f.colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
            chosen = f;
        }
    }
    if (chosen.format != VK_FORMAT_B8G8R8A8_UNORM && chosen.format != VK_FORMAT_R8G8B8A8_UNORM) {
        throw std::runtime_error("An 8-bit UNORM surface is required");
    }
    format_ = chosen.format;
    extent_ =
        cap.currentExtent.width != UINT32_MAX
            ? cap.currentExtent
            : VkExtent2D{
                  std::clamp(uint32_t(w), cap.minImageExtent.width, cap.maxImageExtent.width),
                  std::clamp(uint32_t(h), cap.minImageExtent.height, cap.maxImageExtent.height)};
    uint32_t images = std::max(2u, cap.minImageCount);
    if (cap.maxImageCount) {
        images = std::min(images, cap.maxImageCount);
    }
    if (!(cap.supportedUsageFlags & VK_IMAGE_USAGE_TRANSFER_SRC_BIT)) {
        throw std::runtime_error("Surface does not support screenshot readback");
    }
    VkSwapchainCreateInfoKHR ci{VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR};
    ci.surface = surface_;
    ci.minImageCount = images;
    ci.imageFormat = format_;
    ci.imageColorSpace = chosen.colorSpace;
    ci.imageExtent = extent_;
    ci.imageArrayLayers = 1;
    ci.imageUsage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
    ci.imageSharingMode = VK_SHARING_MODE_EXCLUSIVE;
    ci.preTransform = cap.currentTransform;
    ci.compositeAlpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
    for (auto a : {VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
                   VK_COMPOSITE_ALPHA_PRE_MULTIPLIED_BIT_KHR,
                   VK_COMPOSITE_ALPHA_POST_MULTIPLIED_BIT_KHR,
                   VK_COMPOSITE_ALPHA_INHERIT_BIT_KHR}) {
        if (cap.supportedCompositeAlpha & a) {
            ci.compositeAlpha = a;
            break;
        }
    }
    ci.presentMode = VK_PRESENT_MODE_FIFO_KHR;
    ci.clipped = VK_TRUE;
    check(vkCreateSwapchainKHR(device_, &ci, nullptr, &swapchain_));
    check(vkGetSwapchainImagesKHR(device_, swapchain_, &count, nullptr));
    images_.resize(count);
    check(vkGetSwapchainImagesKHR(device_, swapchain_, &count, images_.data()));
    auto pass = [&](VkFormat format, VkImageLayout final, bool hdr) {
        VkAttachmentDescription at{};
        at.format = format;
        at.samples = VK_SAMPLE_COUNT_1_BIT;
        at.loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR;
        at.storeOp = VK_ATTACHMENT_STORE_OP_STORE;
        at.stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE;
        at.stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE;
        at.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        at.finalLayout = final;
        VkAttachmentReference color{0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL};
        VkSubpassDescription sub{};
        sub.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS;
        sub.colorAttachmentCount = 1;
        sub.pColorAttachments = &color;
        VkSubpassDependency deps[2]{};
        deps[0].srcSubpass = VK_SUBPASS_EXTERNAL;
        deps[0].dstSubpass = 0;
        deps[0].srcStageMask = hdr ? VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT
                                   : VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
        deps[0].dstStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
        deps[0].srcAccessMask = hdr ? VK_ACCESS_SHADER_READ_BIT : 0;
        deps[0].dstAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
        deps[1].srcSubpass = 0;
        deps[1].dstSubpass = VK_SUBPASS_EXTERNAL;
        deps[1].srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
        deps[1].dstStageMask =
            hdr ? VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT : VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT;
        deps[1].srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
        deps[1].dstAccessMask = hdr ? VK_ACCESS_SHADER_READ_BIT : 0;
        VkRenderPassCreateInfo p{VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO};
        p.attachmentCount = 1;
        p.pAttachments = &at;
        p.subpassCount = 1;
        p.pSubpasses = &sub;
        p.dependencyCount = 2;
        p.pDependencies = deps;
        VkRenderPass r;
        check(vkCreateRenderPass(device_, &p, nullptr, &r));
        return r;
    };
    if (!render_pass_) {
        render_pass_ = pass(format_, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, false);
    }
    if (!hdr_pass_) {
        hdr_pass_ = pass(hdr_format_, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, true);
    }
    auto image_view = [&](VkImage image, VkFormat format) {
        VkImageViewCreateInfo v{VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO};
        v.image = image;
        v.viewType = VK_IMAGE_VIEW_TYPE_2D;
        v.format = format;
        v.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
        VkImageView result;
        check(vkCreateImageView(device_, &v, nullptr, &result));
        return result;
    };
    auto framebuffer = [&](VkImageView view, VkRenderPass pass) {
        VkFramebufferCreateInfo f{VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO};
        f.renderPass = pass;
        f.attachmentCount = 1;
        f.pAttachments = &view;
        f.width = extent_.width;
        f.height = extent_.height;
        f.layers = 1;
        VkFramebuffer result;
        check(vkCreateFramebuffer(device_, &f, nullptr, &result));
        return result;
    };
    for (auto image : images_) {
        auto view = image_view(image, format_);
        views_.push_back(view);
        framebuffers_.push_back(framebuffer(view, render_pass_));
        VkSemaphore sem;
        VkSemaphoreCreateInfo s{VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
        check(vkCreateSemaphore(device_, &s, nullptr, &sem));
        presented_.push_back(sem);
    }
    for (auto& f : frames_) {
        VkImageCreateInfo i{VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO};
        i.imageType = VK_IMAGE_TYPE_2D;
        i.format = hdr_format_;
        i.extent = {extent_.width, extent_.height, 1};
        i.mipLevels = 1;
        i.arrayLayers = 1;
        i.samples = VK_SAMPLE_COUNT_1_BIT;
        i.tiling = VK_IMAGE_TILING_OPTIMAL;
        i.usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_SAMPLED_BIT;
        i.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
        check(vkCreateImage(device_, &i, nullptr, &f.hdr));
        VkMemoryRequirements req;
        vkGetImageMemoryRequirements(device_, f.hdr, &req);
        VkMemoryAllocateInfo a{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
        a.allocationSize = req.size;
        a.memoryTypeIndex = memory_type(req.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
        check(vkAllocateMemory(device_, &a, nullptr, &f.hdr_memory));
        check(vkBindImageMemory(device_, f.hdr, f.hdr_memory, 0));
        f.hdr_view = image_view(f.hdr, hdr_format_);
        f.hdr_framebuffer = framebuffer(f.hdr_view, hdr_pass_);
        VkDescriptorSetAllocateInfo da{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO};
        da.descriptorPool = descriptors_;
        da.descriptorSetCount = 1;
        da.pSetLayouts = &tone_set_layout_;
        check(vkAllocateDescriptorSets(device_, &da, &f.tone_set));
        VkDescriptorImageInfo ii{sampler_, f.hdr_view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
        VkWriteDescriptorSet wr{VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET};
        wr.dstSet = f.tone_set;
        wr.dstBinding = 0;
        wr.descriptorCount = 1;
        wr.descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
        wr.pImageInfo = &ii;
        vkUpdateDescriptorSets(device_, 1, &wr, 0, nullptr);
    }
    rebuild_ = false;
}

void Renderer::destroy_swapchain() {
    for (auto& f : frames_) {
        if (f.tone_set) {
            vkFreeDescriptorSets(device_, descriptors_, 1, &f.tone_set);
        }
        if (f.hdr_framebuffer) {
            vkDestroyFramebuffer(device_, f.hdr_framebuffer, nullptr);
        }
        if (f.hdr_view) {
            vkDestroyImageView(device_, f.hdr_view, nullptr);
        }
        if (f.hdr) {
            vkDestroyImage(device_, f.hdr, nullptr);
        }
        if (f.hdr_memory) {
            vkFreeMemory(device_, f.hdr_memory, nullptr);
        }
        f.tone_set = {};
        f.hdr_framebuffer = {};
        f.hdr_view = {};
        f.hdr = {};
        f.hdr_memory = {};
    }
    for (auto f : framebuffers_) {
        vkDestroyFramebuffer(device_, f, nullptr);
    }
    for (auto v : views_) {
        vkDestroyImageView(device_, v, nullptr);
    }
    for (auto s : presented_) {
        vkDestroySemaphore(device_, s, nullptr);
    }
    framebuffers_.clear();
    views_.clear();
    presented_.clear();
    images_.clear();
    if (swapchain_) {
        vkDestroySwapchainKHR(device_, swapchain_, nullptr);
    }
    swapchain_ = {};
}

VkShaderModule Renderer::shader(const char* name) {
    auto p = shaders_ / (std::string(name) + ".spv");
    std::ifstream in(p, std::ios::binary | std::ios::ate);
    if (!in) {
        throw std::runtime_error("Missing SPIR-V: " + p.string());
    }
    auto size = in.tellg();
    if (size <= 0 || size % 4) {
        throw std::runtime_error("Invalid SPIR-V");
    }
    std::vector<uint32_t> code(size_t(size) / 4);
    in.seekg(0);
    in.read((char*)code.data(), size);
    VkShaderModuleCreateInfo ci{VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO};
    ci.codeSize = size_t(size);
    ci.pCode = code.data();
    VkShaderModule module;
    check(vkCreateShaderModule(device_, &ci, nullptr, &module));
    return module;
}

void Renderer::create_pipelines() {
    auto layout = [&](uint32_t size, VkShaderStageFlags stage, bool atmosphere) {
        VkPushConstantRange pc{stage, 0, size};
        VkPipelineLayoutCreateInfo i{VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO};
        i.pushConstantRangeCount = 1;
        i.pPushConstantRanges = &pc;
        VkDescriptorSetLayout layouts[] = {tone_set_layout_, atmosphere_set_layout_};
        i.setLayoutCount = atmosphere ? 2 : 1;
        i.pSetLayouts = layouts;
        VkPipelineLayout l;
        check(vkCreatePipelineLayout(device_, &i, nullptr, &l));
        return l;
    };
    sky_layout_ = layout(128, VK_SHADER_STAGE_FRAGMENT_BIT, true);
    stars_layout_ = layout(128, VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT, true);
    tone_layout_ = layout(8, VK_SHADER_STAGE_FRAGMENT_BIT, false);
    auto pipeline = [&](const char* vert,
                        const char* frag,
                        VkPipelineLayout layout,
                        VkRenderPass pass,
                        bool stars) {
        auto vs = shader(vert), fs = shader(frag);
        VkSpecializationMapEntry filtering{0, 0, sizeof(VkBool32)};
        VkSpecializationInfo specialization{
            1, &filtering, sizeof(VkBool32), &manual_atmosphere_filtering_};
        VkPipelineShaderStageCreateInfo stages[2]{};
        stages[0] = {VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                     nullptr,
                     0,
                     VK_SHADER_STAGE_VERTEX_BIT,
                     vs,
                     "main",
                     nullptr};
        stages[1] = {VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                     nullptr,
                     0,
                     VK_SHADER_STAGE_FRAGMENT_BIT,
                     fs,
                     "main",
                     &specialization};
        VkVertexInputBindingDescription binding{
            0, sizeof(StarInstance), VK_VERTEX_INPUT_RATE_INSTANCE};
        VkVertexInputAttributeDescription attrs[3] = {{0, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 0},
                                                      {1, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 16},
                                                      {2, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 32}};
        VkPipelineVertexInputStateCreateInfo vi{
            VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO};
        if (stars) {
            vi.vertexBindingDescriptionCount = 1;
            vi.pVertexBindingDescriptions = &binding;
            vi.vertexAttributeDescriptionCount = 3;
            vi.pVertexAttributeDescriptions = attrs;
        }
        VkPipelineInputAssemblyStateCreateInfo ia{
            VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO};
        ia.topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
        VkPipelineViewportStateCreateInfo vp{VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO};
        vp.viewportCount = 1;
        vp.scissorCount = 1;
        VkPipelineRasterizationStateCreateInfo rs{
            VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO};
        rs.polygonMode = VK_POLYGON_MODE_FILL;
        rs.cullMode = VK_CULL_MODE_NONE;
        rs.frontFace = VK_FRONT_FACE_COUNTER_CLOCKWISE;
        rs.lineWidth = 1;
        VkPipelineMultisampleStateCreateInfo ms{
            VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO};
        ms.rasterizationSamples = VK_SAMPLE_COUNT_1_BIT;
        VkPipelineColorBlendAttachmentState attachment{};
        attachment.colorWriteMask = 15;
        attachment.blendEnable = stars;
        // Premultiplied emission: points have zero opacity and add light;
        // resolved disks retain coverage alpha and occlude the background.
        attachment.srcColorBlendFactor = VK_BLEND_FACTOR_ONE;
        attachment.dstColorBlendFactor = VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA;
        attachment.colorBlendOp = VK_BLEND_OP_ADD;
        attachment.srcAlphaBlendFactor = VK_BLEND_FACTOR_ONE;
        attachment.dstAlphaBlendFactor = VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA;
        attachment.alphaBlendOp = VK_BLEND_OP_ADD;
        VkPipelineColorBlendStateCreateInfo blend{
            VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO};
        blend.attachmentCount = 1;
        blend.pAttachments = &attachment;
        VkDynamicState dynamic[] = {VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR};
        VkPipelineDynamicStateCreateInfo ds{VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO};
        ds.dynamicStateCount = 2;
        ds.pDynamicStates = dynamic;
        VkGraphicsPipelineCreateInfo pi{VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO};
        pi.stageCount = 2;
        pi.pStages = stages;
        pi.pVertexInputState = &vi;
        pi.pInputAssemblyState = &ia;
        pi.pViewportState = &vp;
        pi.pRasterizationState = &rs;
        pi.pMultisampleState = &ms;
        pi.pColorBlendState = &blend;
        pi.pDynamicState = &ds;
        pi.layout = layout;
        pi.renderPass = pass;
        VkPipeline result;
        auto status = vkCreateGraphicsPipelines(device_, VK_NULL_HANDLE, 1, &pi, nullptr, &result);
        vkDestroyShaderModule(device_, vs, nullptr);
        vkDestroyShaderModule(device_, fs, nullptr);
        check(status);
        return result;
    };
    sky_pipeline_ = pipeline("sky.vert", "sky.frag", sky_layout_, hdr_pass_, false);
    stars_pipeline_ = pipeline("stars.vert", "stars.frag", stars_layout_, hdr_pass_, true);
    tone_pipeline_ = pipeline("sky.vert", "tonemap.frag", tone_layout_, render_pass_, false);
}

void Renderer::render(const RenderScene& s,
                      ImDrawData* ui,
                      const std::filesystem::path& screenshot) {
    int width, height;
    SDL_GetWindowSizeInPixels(window_, &width, &height);
    if (width <= 0 || height <= 0) {
        return;
    }
    if (rebuild_ || uint32_t(width) != extent_.width || uint32_t(height) != extent_.height) {
        check(vkDeviceWaitIdle(device_));
        destroy_swapchain();
        create_swapchain();
        if (imgui_ready_) {
            ImGui_ImplVulkan_SetMinImageCount(2);
        }
    }
    auto& f = frames_[frame_];
    check(vkWaitForFences(device_, 1, &f.fence, VK_TRUE, UINT64_MAX));
    const auto& atmosphere = *atmospheres_[s.atmosphere_preset];
    const float twilight = float(photometry::twilight_gain(-asin(s.geometric_sun.z) / rad));
    const float pollution = float(photometry::pollution_luminance(s.pollution));
    const auto solar = atmosphere.irradiance(s.height_km, float(s.geometric_sun.z));
    auto lunar = atmosphere.irradiance(s.height_km, float(s.geometric_moon.z));
    constexpr float luminance[] = {.2126f, .7152f, .0722f};
    const auto transmission = atmosphere.transmittance(s.height_km, float(s.geometric_moon.z));
    const double visible_moon = std::clamp((s.geometric_moon.z + .0047) / .0094, 0., 1.);
    double moon_transmission = 0, old_lunar_mean = 0;
    for (int i = 0; i < 3; ++i) {
        moon_transmission +=
            luminance[i] * photometry::solar_rgb[i] * transmission[i] / photometry::solar_lux;
        lunar[i] *= s.lunar_flux;
        old_lunar_mean += luminance[i] * lunar[i] / pi;
    }
    double lunar_mean = 0;
    // Deterministic cosine-weighted hemisphere integral of the same empirical
    // lunar law used by the fragment shader. It is independent of the camera.
    if (s.atmosphere && s.lunar_flux > 1e-10 && visible_moon > 0) {
        for (int j = 0; j < 12; ++j) {
            const double mu = (j + .5) / 12;
            const auto t = atmosphere.transmittance(s.height_km, float(mu));
            double view_t = 0;
            for (int c = 0; c < 3; ++c) {
                view_t += luminance[c] * photometry::solar_rgb[c] * t[c] / photometry::solar_lux;
            }
            for (int k = 0; k < 24; ++k) {
                const double cosine =
                    mu * s.geometric_moon.z +
                    sqrt((1 - mu * mu) *
                         std::max(0., 1 - s.geometric_moon.z * s.geometric_moon.z)) *
                        cos(2 * pi * (k + .5) / 24);
                lunar_mean += 2 * mu / (12 * 24) *
                              photometry::lunar_sky_luminance(s.lunar_flux * photometry::solar_lux,
                                                              acos(std::clamp(cosine, -1., 1.)),
                                                              moon_transmission,
                                                              view_t);
            }
        }
    }
    lunar_mean = std::lerp(old_lunar_mean, lunar_mean, visible_moon);
    for (auto& c : lunar) {
        c *= float(lunar_mean / std::max(1e-20, old_lunar_mean));
    }
    float adaptation = 300;
    if (s.atmosphere && s.auto_exposure) {
        const double pollution_mean_ratio = (1.5 + .5 * exp(-pi)) / (1 + 2 * exp(-pi));
        double mean = photometry::night_floor + pollution * pollution_mean_ratio;
        const double moon_illuminance =
            s.lunar_flux * photometry::solar_lux * moon_transmission * visible_moon;
        for (int i = 0; i < 3; ++i) {
            mean += luminance[i] * (solar[i] * s.solar_flux * twilight + lunar[i]) / float(pi);
        }
        adaptation = float(photometry::exposure_gain(mean, moon_illuminance));
    }
    effective_exposure_ = adaptation * s.exposure;
    const float atmosphere_uniform[] = {float(s.geometric_sun.x),
                                        float(s.geometric_sun.y),
                                        float(s.geometric_sun.z),
                                        s.solar_flux,
                                        float(s.geometric_moon.x),
                                        float(s.geometric_moon.y),
                                        float(s.geometric_moon.z),
                                        s.lunar_flux,
                                        s.height_km,
                                        float(s.atmosphere_preset),
                                        s.auto_exposure ? 1.f : 0.f,
                                        adaptation,
                                        twilight,
                                        float(photometry::night_floor),
                                        pollution,
                                        background_radiance_scale,
                                        lunar[0],
                                        lunar[1],
                                        lunar[2],
                                        0};
    memcpy(f.atmosphere.mapped, atmosphere_uniform, sizeof(atmosphere_uniform));
    uint32_t image;
    auto acquired =
        vkAcquireNextImageKHR(device_, swapchain_, UINT64_MAX, f.acquired, VK_NULL_HANDLE, &image);
    if (acquired == VK_ERROR_OUT_OF_DATE_KHR) {
        rebuild_ = true;
        return;
    }
    if (acquired == VK_SUBOPTIMAL_KHR) {
        rebuild_ = true;
    } else {
        check(acquired);
    }
    VkDeviceSize bytes =
        std::max(VkDeviceSize(48), VkDeviceSize(s.points.size() * sizeof(StarInstance)));
    if (bytes > f.instances.size) {
        release(f.instances);
        f.instances = buffer(bytes * 2, VK_BUFFER_USAGE_VERTEX_BUFFER_BIT);
    }
    if (!s.points.empty()) {
        memcpy(f.instances.mapped, s.points.data(), s.points.size() * sizeof(StarInstance));
    }
    Buffer readback;
    if (!screenshot.empty()) {
        readback = buffer(VkDeviceSize(extent_.width) * extent_.height * 4,
                          VK_BUFFER_USAGE_TRANSFER_DST_BIT);
    }
    check(vkResetCommandPool(device_, f.pool, 0));
    VkCommandBufferBeginInfo begin{VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
    begin.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    check(vkBeginCommandBuffer(f.command, &begin));
    VkViewport vp{0, 0, float(extent_.width), float(extent_.height), 0, 1};
    VkRect2D scissor{{0, 0}, extent_};
    vkCmdSetViewport(f.command, 0, 1, &vp);
    vkCmdSetScissor(f.command, 0, 1, &scissor);
    VkClearValue clear{};
    clear.color.float32[3] = 1;
    VkRenderPassBeginInfo pass{VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO};
    pass.renderPass = hdr_pass_;
    pass.framebuffer = f.hdr_framebuffer;
    pass.renderArea = scissor;
    pass.clearValueCount = 1;
    pass.pClearValues = &clear;
    vkCmdBeginRenderPass(f.command, &pass, VK_SUBPASS_CONTENTS_INLINE);

    struct Parameters {
        float right[4], up[4], forward[4], sun[4], moon[4], options[4], viewport[4], galactic[4];
    } p{};

    static_assert(sizeof(Parameters) == 128);

    auto copy = [](Vec3 v, float* a) {
        a[0] = float(v.x);
        a[1] = float(v.y);
        a[2] = float(v.z);
    };
    copy(s.camera.right(), p.right);
    p.right[3] = float(s.camera.pixels_per_radian());
    copy(s.camera.up(), p.up);
    p.up[3] = float(s.camera.fov / 2);
    copy(s.camera.forward(), p.forward);
    p.forward[3] = float(s.camera.projection);
    copy(s.sun, p.sun);
    p.sun[3] = s.atmosphere ? 1 : 0;
    copy(s.moon, p.moon);
    p.moon[3] = s.ground ? 1 : 0;
    p.options[0] = s.pollution;
    p.options[1] = s.moon_phase;
    p.options[2] = s.milky_way ? 1 : 0;
    p.options[3] = s.extinction;
    std::copy(s.galactic_rotation.begin(), s.galactic_rotation.end(), p.galactic);
    p.viewport[0] = float(s.camera.width);
    p.viewport[1] = float(s.camera.height);
    p.viewport[2] = float(extent_.width);
    p.viewport[3] = float(extent_.height);
    vkCmdBindPipeline(f.command, VK_PIPELINE_BIND_POINT_GRAPHICS, sky_pipeline_);
    vkCmdBindDescriptorSets(f.command,
                            VK_PIPELINE_BIND_POINT_GRAPHICS,
                            sky_layout_,
                            1,
                            1,
                            &f.atmosphere_set,
                            0,
                            nullptr);
    vkCmdBindDescriptorSets(f.command,
                            VK_PIPELINE_BIND_POINT_GRAPHICS,
                            sky_layout_,
                            0,
                            1,
                            &background_set_,
                            0,
                            nullptr);
    vkCmdPushConstants(f.command, sky_layout_, VK_SHADER_STAGE_FRAGMENT_BIT, 0, sizeof(p), &p);
    vkCmdDraw(f.command, 3, 1, 0, 0);
    if (!s.points.empty()) {
        vkCmdBindPipeline(f.command, VK_PIPELINE_BIND_POINT_GRAPHICS, stars_pipeline_);
        vkCmdBindDescriptorSets(f.command,
                                VK_PIPELINE_BIND_POINT_GRAPHICS,
                                stars_layout_,
                                1,
                                1,
                                &f.atmosphere_set,
                                0,
                                nullptr);
        VkDeviceSize offset = 0;
        vkCmdBindVertexBuffers(f.command, 0, 1, &f.instances.handle, &offset);
        vkCmdPushConstants(f.command,
                           stars_layout_,
                           VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT,
                           0,
                           sizeof(p),
                           &p);
        vkCmdDraw(f.command, 6, uint32_t(s.points.size()), 0, 0);
    }
    vkCmdEndRenderPass(f.command);
    // Make the HDR store visible before tone mapping. The explicit barrier is
    // also needed by MoltenVK's tile renderer with the manual LUT filter path;
    // relying on the external subpass dependency alone produced stale tiles.
    VkMemoryBarrier hdr_ready{VK_STRUCTURE_TYPE_MEMORY_BARRIER};
    hdr_ready.srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
    hdr_ready.dstAccessMask = VK_ACCESS_SHADER_READ_BIT;
    vkCmdPipelineBarrier(f.command,
                         VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
                         VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,
                         0,
                         1,
                         &hdr_ready,
                         0,
                         nullptr,
                         0,
                         nullptr);
    pass.renderPass = render_pass_;
    pass.framebuffer = framebuffers_[image];
    vkCmdBeginRenderPass(f.command, &pass, VK_SUBPASS_CONTENTS_INLINE);
    vkCmdBindPipeline(f.command, VK_PIPELINE_BIND_POINT_GRAPHICS, tone_pipeline_);
    vkCmdBindDescriptorSets(
        f.command, VK_PIPELINE_BIND_POINT_GRAPHICS, tone_layout_, 0, 1, &f.tone_set, 0, nullptr);
    const float display[] = {s.exposure, adaptation};
    vkCmdPushConstants(
        f.command, tone_layout_, VK_SHADER_STAGE_FRAGMENT_BIT, 0, sizeof(display), display);
    vkCmdDraw(f.command, 3, 1, 0, 0);
    if (ui) {
        ImGui_ImplVulkan_RenderDrawData(ui, f.command);
    }
    vkCmdEndRenderPass(f.command);
    if (readback.handle) {
        VkImageMemoryBarrier b{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
        b.srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
        b.dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT;
        b.oldLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
        b.newLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
        b.srcQueueFamilyIndex = b.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
        b.image = images_[image];
        b.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
        vkCmdPipelineBarrier(f.command,
                             VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             0,
                             0,
                             nullptr,
                             0,
                             nullptr,
                             1,
                             &b);
        VkBufferImageCopy cp{};
        cp.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1};
        cp.imageExtent = {extent_.width, extent_.height, 1};
        vkCmdCopyImageToBuffer(f.command,
                               images_[image],
                               VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                               readback.handle,
                               1,
                               &cp);
        b.srcAccessMask = VK_ACCESS_TRANSFER_READ_BIT;
        b.dstAccessMask = 0;
        b.oldLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
        b.newLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
        vkCmdPipelineBarrier(f.command,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
                             0,
                             0,
                             nullptr,
                             0,
                             nullptr,
                             1,
                             &b);
    }
    check(vkEndCommandBuffer(f.command));
    VkPipelineStageFlags wait = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
    VkSubmitInfo submit{VK_STRUCTURE_TYPE_SUBMIT_INFO};
    submit.waitSemaphoreCount = 1;
    submit.pWaitSemaphores = &f.acquired;
    submit.pWaitDstStageMask = &wait;
    submit.commandBufferCount = 1;
    submit.pCommandBuffers = &f.command;
    submit.signalSemaphoreCount = 1;
    submit.pSignalSemaphores = &presented_[image];
    check(vkResetFences(device_, 1, &f.fence));
    check(vkQueueSubmit(queue_, 1, &submit, f.fence));
    VkPresentInfoKHR present{VK_STRUCTURE_TYPE_PRESENT_INFO_KHR};
    present.waitSemaphoreCount = 1;
    present.pWaitSemaphores = &presented_[image];
    present.swapchainCount = 1;
    present.pSwapchains = &swapchain_;
    present.pImageIndices = &image;
    auto status = vkQueuePresentKHR(queue_, &present);
    if (status == VK_ERROR_OUT_OF_DATE_KHR || status == VK_SUBOPTIMAL_KHR) {
        rebuild_ = true;
    } else {
        check(status);
    }
    if (readback.handle) {
        check(vkWaitForFences(device_, 1, &f.fence, VK_TRUE, UINT64_MAX));
        auto* rgba = static_cast<unsigned char*>(readback.mapped);
        if (format_ == VK_FORMAT_B8G8R8A8_UNORM) {
            for (size_t i = 0; i < size_t(extent_.width) * extent_.height; i++) {
                std::swap(rgba[i * 4], rgba[i * 4 + 2]);
            }
        }
        if (!screenshot.parent_path().empty()) {
            std::filesystem::create_directories(screenshot.parent_path());
        }
        png_image image{};
        image.version = PNG_IMAGE_VERSION;
        image.width = extent_.width;
        image.height = extent_.height;
        image.format = PNG_FORMAT_RGBA;
        bool ok = png_image_write_to_file(&image, screenshot.string().c_str(), 0, rgba, 0, nullptr);
        std::string error = image.message;
        png_image_free(&image);
        release(readback);
        if (!ok) {
            throw std::runtime_error("PNG export failed: " + error);
        }
    }
    frame_ = (frame_ + 1) % 2;
}

void Renderer::cleanup() {
    if (device_) {
        vkDeviceWaitIdle(device_);
    }
    if (imgui_ready_) {
        ImGui_ImplVulkan_Shutdown();
        imgui_ready_ = false;
    }
    if (device_) {
        destroy_swapchain();
        for (auto& f : frames_) {
            release(f.instances);
            release(f.atmosphere);
            if (f.acquired) {
                vkDestroySemaphore(device_, f.acquired, nullptr);
            }
            if (f.fence) {
                vkDestroyFence(device_, f.fence, nullptr);
            }
            if (f.pool) {
                vkDestroyCommandPool(device_, f.pool, nullptr);
            }
        }
        for (auto p : {sky_pipeline_, stars_pipeline_, tone_pipeline_}) {
            if (p) {
                vkDestroyPipeline(device_, p, nullptr);
            }
        }
        for (auto l : {sky_layout_, stars_layout_, tone_layout_}) {
            if (l) {
                vkDestroyPipelineLayout(device_, l, nullptr);
            }
        }
        if (sampler_) {
            vkDestroySampler(device_, sampler_, nullptr);
        }
        if (background_sampler_) {
            vkDestroySampler(device_, background_sampler_, nullptr);
        }
        if (background_view_) {
            vkDestroyImageView(device_, background_view_, nullptr);
        }
        if (background_) {
            vkDestroyImage(device_, background_, nullptr);
        }
        if (background_memory_) {
            vkFreeMemory(device_, background_memory_, nullptr);
        }
        if (tone_set_layout_) {
            vkDestroyDescriptorSetLayout(device_, tone_set_layout_, nullptr);
        }
        if (atmosphere_set_layout_) {
            vkDestroyDescriptorSetLayout(device_, atmosphere_set_layout_, nullptr);
        }
        if (descriptors_) {
            vkDestroyDescriptorPool(device_, descriptors_, nullptr);
        }
        if (hdr_pass_) {
            vkDestroyRenderPass(device_, hdr_pass_, nullptr);
        }
        if (render_pass_) {
            vkDestroyRenderPass(device_, render_pass_, nullptr);
        }
        for (auto& atmosphere : atmospheres_) {
            atmosphere.reset();
        }
        vkDestroyDevice(device_, nullptr);
        device_ = {};
    }
    if (surface_) {
        vkDestroySurfaceKHR(instance_, surface_, nullptr);
    }
    if (debug_) {
        auto destroy = (PFN_vkDestroyDebugUtilsMessengerEXT)vkGetInstanceProcAddr(
            instance_, "vkDestroyDebugUtilsMessengerEXT");
        destroy(instance_, debug_, nullptr);
    }
    if (instance_) {
        vkDestroyInstance(instance_, nullptr);
    }
    instance_ = {};
}

Renderer::~Renderer() {
    cleanup();
}
} // namespace astro
