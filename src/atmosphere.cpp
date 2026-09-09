#include "astro/atmosphere.hpp"
#include <algorithm>
#include <bit>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <vector>

namespace astro {
namespace {
void check(VkResult result) {
    if (result != VK_SUCCESS) {
        throw std::runtime_error("Atmosphere Vulkan error: " + std::to_string(result));
    }
}

constexpr VkExtent3D dimensions[] = {{256, 64, 1},
                                     {256, 128, 32},
                                     {256, 128, 32},
                                     {64, 16, 1},
                                     {64, 16, 1},
                                     {256, 128, 32},
                                     {256, 128, 32},
                                     {256, 128, 32},
                                     {256, 128, 32}};

struct CacheHeader {
    char magic[8] = {'A', 'S', 'T', 'R', 'A', 'A', 'T', '2'};
    char model[68] = ASTRA_ATMOSPHERE_ID;
    uint32_t preset{}, orders{}, groups = ASTRA_SPECTRAL_GROUPS;
    uint32_t crc{};
};

static_assert(sizeof(CacheHeader) == 92);
static_assert(std::endian::native == std::endian::little);

uint32_t checksum(const std::vector<float>& values) {
    static const auto table = [] {
        std::array<uint32_t, 256> result{};
        for (uint32_t i = 0; i < result.size(); ++i) {
            uint32_t crc = i;
            for (int bit = 0; bit < 8; ++bit) {
                crc = (crc >> 1) ^ ((crc & 1) ? 0xedb88320u : 0u);
            }
            result[i] = crc;
        }
        return result;
    }();
    uint32_t crc = 0xffffffffu;
    const auto* bytes = reinterpret_cast<const unsigned char*>(values.data());
    for (size_t i = 0; i < values.size() * sizeof(float); ++i) {
        crc = table[(crc ^ bytes[i]) & 255] ^ (crc >> 8);
    }
    return crc ^ 0xffffffffu;
}

VkDeviceSize image_bytes(unsigned i) {
    const auto d = dimensions[i];
    return VkDeviceSize(d.width) * d.height * d.depth * 4 * sizeof(float);
}
} // namespace

uint32_t AtmosphereLut::memory_type(uint32_t bits, VkMemoryPropertyFlags flags) const {
    VkPhysicalDeviceMemoryProperties properties;
    vkGetPhysicalDeviceMemoryProperties(physical_, &properties);
    for (uint32_t i = 0; i < properties.memoryTypeCount; ++i) {
        if ((bits & (1u << i)) && (properties.memoryTypes[i].propertyFlags & flags) == flags) {
            return i;
        }
    }
    throw std::runtime_error("No memory type for atmosphere tables");
}

void AtmosphereLut::create_image(unsigned index) {
    auto& image = images_[index];
    image.extent = dimensions[index];
    VkImageCreateInfo info{VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO};
    info.imageType = image.extent.depth > 1 ? VK_IMAGE_TYPE_3D : VK_IMAGE_TYPE_2D;
    info.format = VK_FORMAT_R32G32B32A32_SFLOAT;
    info.extent = image.extent;
    info.mipLevels = info.arrayLayers = 1;
    info.samples = VK_SAMPLE_COUNT_1_BIT;
    info.tiling = VK_IMAGE_TILING_OPTIMAL;
    info.usage = VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_STORAGE_BIT |
                 VK_IMAGE_USAGE_TRANSFER_SRC_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT;
    check(vkCreateImage(device_, &info, nullptr, &image.handle));
    VkMemoryRequirements requirements;
    vkGetImageMemoryRequirements(device_, image.handle, &requirements);
    VkMemoryAllocateInfo allocation{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
    allocation.allocationSize = requirements.size;
    allocation.memoryTypeIndex =
        memory_type(requirements.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
    check(vkAllocateMemory(device_, &allocation, nullptr, &image.memory));
    check(vkBindImageMemory(device_, image.handle, image.memory, 0));
    VkImageViewCreateInfo view{VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO};
    view.image = image.handle;
    view.viewType = image.extent.depth > 1 ? VK_IMAGE_VIEW_TYPE_3D : VK_IMAGE_VIEW_TYPE_2D;
    view.format = info.format;
    view.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    check(vkCreateImageView(device_, &view, nullptr, &image.view));
}

void AtmosphereLut::begin() {
    check(vkResetCommandPool(device_, pool_, 0));
    VkCommandBufferBeginInfo info{VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
    info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    check(vkBeginCommandBuffer(command_, &info));
}

void AtmosphereLut::submit() {
    check(vkEndCommandBuffer(command_));
    check(vkResetFences(device_, 1, &fence_));
    VkSubmitInfo info{VK_STRUCTURE_TYPE_SUBMIT_INFO};
    info.commandBufferCount = 1;
    info.pCommandBuffers = &command_;
    check(vkQueueSubmit(queue_, 1, &info, fence_));
    check(vkWaitForFences(device_, 1, &fence_, VK_TRUE, UINT64_MAX));
}

AtmosphereLut::AtmosphereLut(VkPhysicalDevice physical,
                             VkDevice device,
                             VkQueue queue,
                             uint32_t family,
                             const std::filesystem::path& shaders,
                             const std::filesystem::path& cache,
                             unsigned preset,
                             bool prepare,
                             unsigned orders)
    : physical_(physical), device_(device), queue_(queue) {
    try {
        if (preset > 1 || orders < 2 || orders > 16) {
            throw std::runtime_error("Invalid atmosphere preset/scattering order");
        }
        VkFormatProperties format;
        vkGetPhysicalDeviceFormatProperties(physical_, VK_FORMAT_R32G32B32A32_SFLOAT, &format);
        const auto required =
            VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT | VK_FORMAT_FEATURE_STORAGE_IMAGE_BIT;
        if ((format.optimalTilingFeatures & required) != required) {
            throw std::runtime_error("Physical atmosphere requires RGBA32F sampled/storage images");
        }
        VkSamplerCreateInfo sampler{VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO};
        const bool linear =
            (format.optimalTilingFeatures & VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT) &&
            !std::getenv("ASTRA_ATMOSPHERE_MANUAL_FILTER");
        manual_filtering_ = !linear;
        sampler.magFilter = sampler.minFilter = linear ? VK_FILTER_LINEAR : VK_FILTER_NEAREST;
        sampler.addressModeU = sampler.addressModeV = sampler.addressModeW =
            VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
        check(vkCreateSampler(device_, &sampler, nullptr, &sampler_));
        VkCommandPoolCreateInfo pool{VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO};
        pool.queueFamilyIndex = family;
        check(vkCreateCommandPool(device_, &pool, nullptr, &pool_));
        VkCommandBufferAllocateInfo command{VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO};
        command.commandPool = pool_;
        command.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
        command.commandBufferCount = 1;
        check(vkAllocateCommandBuffers(device_, &command, &command_));
        VkFenceCreateInfo fence{VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
        check(vkCreateFence(device_, &fence, nullptr, &fence_));
        for (unsigned i = 0; i < (prepare ? 9u : 4u); ++i) {
            create_image(i);
        }
        begin();
        for (const auto& image : images_) {
            if (!image.handle) {
                continue;
            }
            VkImageMemoryBarrier barrier{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
            barrier.oldLayout = VK_IMAGE_LAYOUT_UNDEFINED;
            barrier.newLayout = VK_IMAGE_LAYOUT_GENERAL;
            barrier.dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
            barrier.srcQueueFamilyIndex = barrier.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
            barrier.image = image.handle;
            barrier.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
            vkCmdPipelineBarrier(command_,
                                 VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
                                 VK_PIPELINE_STAGE_TRANSFER_BIT,
                                 0,
                                 0,
                                 nullptr,
                                 0,
                                 nullptr,
                                 1,
                                 &barrier);
            VkClearColorValue zero{};
            vkCmdClearColorImage(command_,
                                 image.handle,
                                 VK_IMAGE_LAYOUT_GENERAL,
                                 &zero,
                                 1,
                                 &barrier.subresourceRange);
        }
        VkMemoryBarrier ready{VK_STRUCTURE_TYPE_MEMORY_BARRIER};
        ready.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        ready.dstAccessMask =
            VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_SHADER_WRITE_BIT | VK_ACCESS_TRANSFER_WRITE_BIT;
        vkCmdPipelineBarrier(command_,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                             0,
                             1,
                             &ready,
                             0,
                             nullptr,
                             0,
                             nullptr);
        submit();
        if (prepare) {
            generate(shaders, preset, orders);
        }
        transfer(cache, prepare, preset, orders);
    } catch (...) {
        cleanup();
        throw;
    }
}

void AtmosphereLut::transfer(const std::filesystem::path& path,
                             bool write,
                             unsigned preset,
                             unsigned orders) {
    CacheHeader header;
    header.preset = preset;
    header.orders = orders;
    VkDeviceSize size = 0;
    for (unsigned i = 0; i < 4; ++i) {
        size += image_bytes(i);
    }
    std::vector<float> data(size / sizeof(float));
    if (!write) {
        std::ifstream file(path, std::ios::binary);
        CacheHeader actual{};
        file.read(reinterpret_cast<char*>(&actual), sizeof(actual));
        if (!file || std::memcmp(actual.magic, header.magic, 8) ||
            std::memcmp(actual.model, header.model, 65) || actual.preset != preset ||
            actual.orders != orders || actual.groups != header.groups) {
            throw std::runtime_error("Missing/incompatible atmosphere cache: " + path.string() +
                                     "; run build/astra_atmosphere data/atmosphere");
        }
        file.read(reinterpret_cast<char*>(data.data()), size);
        if (!file || file.peek() != std::char_traits<char>::eof()) {
            throw std::runtime_error("Invalid atmosphere cache length: " + path.string());
        }
        if (checksum(data) != actual.crc) {
            throw std::runtime_error("Atmosphere cache checksum mismatch: " + path.string());
        }
    }
    VkBuffer buffer{};
    VkDeviceMemory memory{};
    void* mapped = nullptr;
    try {
        VkBufferCreateInfo info{VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO};
        info.size = size;
        info.usage = VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT;
        check(vkCreateBuffer(device_, &info, nullptr, &buffer));
        VkMemoryRequirements requirements;
        vkGetBufferMemoryRequirements(device_, buffer, &requirements);
        VkMemoryAllocateInfo allocation{VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
        allocation.allocationSize = requirements.size;
        allocation.memoryTypeIndex =
            memory_type(requirements.memoryTypeBits,
                        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
        check(vkAllocateMemory(device_, &allocation, nullptr, &memory));
        check(vkBindBufferMemory(device_, buffer, memory, 0));
        check(vkMapMemory(device_, memory, 0, size, 0, &mapped));
        if (!write) {
            std::memcpy(mapped, data.data(), size);
        }
        begin();
        VkMemoryBarrier before{VK_STRUCTURE_TYPE_MEMORY_BARRIER};
        before.srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT | VK_ACCESS_TRANSFER_WRITE_BIT;
        before.dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT | VK_ACCESS_TRANSFER_WRITE_BIT;
        vkCmdPipelineBarrier(command_,
                             VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             0,
                             1,
                             &before,
                             0,
                             nullptr,
                             0,
                             nullptr);
        VkDeviceSize offset = 0;
        for (unsigned i = 0; i < 4; ++i) {
            VkBufferImageCopy copy{};
            copy.bufferOffset = offset;
            copy.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1};
            copy.imageExtent = images_[i].extent;
            if (write) {
                vkCmdCopyImageToBuffer(
                    command_, images_[i].handle, VK_IMAGE_LAYOUT_GENERAL, buffer, 1, &copy);
            } else {
                vkCmdCopyBufferToImage(
                    command_, buffer, images_[i].handle, VK_IMAGE_LAYOUT_GENERAL, 1, &copy);
            }
            offset += image_bytes(i);
        }
        VkMemoryBarrier after{VK_STRUCTURE_TYPE_MEMORY_BARRIER};
        after.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        after.dstAccessMask = write ? VK_ACCESS_HOST_READ_BIT : VK_ACCESS_SHADER_READ_BIT;
        vkCmdPipelineBarrier(command_,
                             VK_PIPELINE_STAGE_TRANSFER_BIT,
                             write ? VK_PIPELINE_STAGE_HOST_BIT
                                   : VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                             0,
                             1,
                             &after,
                             0,
                             nullptr,
                             0,
                             nullptr);
        submit();
        if (write) {
            std::memcpy(data.data(), mapped, size);
        }
        for (float value : data) {
            if (!std::isfinite(value)) {
                throw std::runtime_error("Non-finite atmosphere table value");
            }
        }
        std::copy(data.end() - irradiance_.size(), data.end(), irradiance_.begin());
        if (write) {
            header.crc = checksum(data);
            std::filesystem::create_directories(path.parent_path());
            auto partial = path;
            partial += ".partial";
            std::ofstream file(partial, std::ios::binary);
            file.write(reinterpret_cast<const char*>(&header), sizeof(header));
            file.write(reinterpret_cast<const char*>(data.data()), size);
            file.close();
            if (!file) {
                throw std::runtime_error("Cannot write atmosphere cache");
            }
            std::filesystem::rename(partial, path);
            std::cout << "Atmosphere cache: " << path << " (" << size << " bytes)\n";
        }
    } catch (...) {
        if (mapped) {
            vkUnmapMemory(device_, memory);
        }
        if (buffer) {
            vkDestroyBuffer(device_, buffer, nullptr);
        }
        if (memory) {
            vkFreeMemory(device_, memory, nullptr);
        }
        throw;
    }
    vkUnmapMemory(device_, memory);
    vkDestroyBuffer(device_, buffer, nullptr);
    vkFreeMemory(device_, memory, nullptr);
}

void AtmosphereLut::generate(const std::filesystem::path& shaders,
                             unsigned preset,
                             unsigned orders) {
    VkDescriptorPool descriptors{};
    VkDescriptorSetLayout set_layout{};
    VkPipelineLayout layout{};
    VkPipeline pipeline{};
    VkShaderModule shader{};
    auto release = [&] {
        if (pipeline) {
            vkDestroyPipeline(device_, pipeline, nullptr);
        }
        if (shader) {
            vkDestroyShaderModule(device_, shader, nullptr);
        }
        if (layout) {
            vkDestroyPipelineLayout(device_, layout, nullptr);
        }
        if (descriptors) {
            vkDestroyDescriptorPool(device_, descriptors, nullptr);
        }
        if (set_layout) {
            vkDestroyDescriptorSetLayout(device_, set_layout, nullptr);
        }
    };
    try {
        std::array<VkDescriptorSetLayoutBinding, 18> bindings{};
        for (unsigned i = 0; i < bindings.size(); ++i) {
            bindings[i] = {i,
                           i < 9 ? VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER
                                 : VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
                           1,
                           VK_SHADER_STAGE_COMPUTE_BIT,
                           nullptr};
        }
        VkDescriptorSetLayoutCreateInfo sl{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO};
        sl.bindingCount = bindings.size();
        sl.pBindings = bindings.data();
        check(vkCreateDescriptorSetLayout(device_, &sl, nullptr, &set_layout));
        VkDescriptorPoolSize sizes[] = {{VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 9},
                                        {VK_DESCRIPTOR_TYPE_STORAGE_IMAGE, 9}};
        VkDescriptorPoolCreateInfo dp{VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO};
        dp.maxSets = 1;
        dp.poolSizeCount = 2;
        dp.pPoolSizes = sizes;
        check(vkCreateDescriptorPool(device_, &dp, nullptr, &descriptors));
        VkDescriptorSetAllocateInfo allocation{VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO};
        allocation.descriptorPool = descriptors;
        allocation.descriptorSetCount = 1;
        allocation.pSetLayouts = &set_layout;
        VkDescriptorSet set;
        check(vkAllocateDescriptorSets(device_, &allocation, &set));
        std::array<VkDescriptorImageInfo, 18> infos;
        std::array<VkWriteDescriptorSet, 18> writes{};
        for (unsigned i = 0; i < 18; ++i) {
            infos[i] = {
                i < 9 ? sampler_ : VK_NULL_HANDLE, images_[i % 9].view, VK_IMAGE_LAYOUT_GENERAL};
            writes[i] = {VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET};
            writes[i].dstSet = set;
            writes[i].dstBinding = i;
            writes[i].descriptorCount = 1;
            writes[i].descriptorType = bindings[i].descriptorType;
            writes[i].pImageInfo = &infos[i];
        }
        vkUpdateDescriptorSets(device_, writes.size(), writes.data(), 0, nullptr);
        VkPushConstantRange range{VK_SHADER_STAGE_COMPUTE_BIT, 0, 20};
        VkPipelineLayoutCreateInfo pl{VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO};
        pl.setLayoutCount = 1;
        pl.pSetLayouts = &set_layout;
        pl.pushConstantRangeCount = 1;
        pl.pPushConstantRanges = &range;
        check(vkCreatePipelineLayout(device_, &pl, nullptr, &layout));
        std::ifstream stream(shaders / "atmosphere.comp.spv", std::ios::binary | std::ios::ate);
        if (!stream) {
            throw std::runtime_error("Missing atmosphere compute shader");
        }
        const auto length = stream.tellg();
        std::vector<uint32_t> code(size_t(length) / 4);
        stream.seekg(0);
        stream.read(reinterpret_cast<char*>(code.data()), length);
        VkShaderModuleCreateInfo module{VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO};
        module.codeSize = code.size() * 4;
        module.pCode = code.data();
        check(vkCreateShaderModule(device_, &module, nullptr, &shader));
        VkComputePipelineCreateInfo cp{VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO};
        cp.layout = layout;
        cp.stage = {VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO};
        cp.stage.stage = VK_SHADER_STAGE_COMPUTE_BIT;
        cp.stage.module = shader;
        cp.stage.pName = "main";
        VkSpecializationMapEntry filtering{0, 0, sizeof(VkBool32)};
        VkSpecializationInfo specialization{1, &filtering, sizeof(VkBool32), &manual_filtering_};
        cp.stage.pSpecializationInfo = &specialization;
        check(vkCreateComputePipelines(device_, VK_NULL_HANDLE, 1, &cp, nullptr, &pipeline));
        auto dispatch = [&](int step, int order, int group) {
            const unsigned target = step == 0 ? 0 : (step == 1 || step == 4 ? 3 : 1);
            const auto extent = dimensions[target];
            // Separate submissions bound driver watchdog work during offline preparation.
            for (unsigned z = 0; z < extent.depth; z += 4) {
                begin();
                vkCmdBindPipeline(command_, VK_PIPELINE_BIND_POINT_COMPUTE, pipeline);
                vkCmdBindDescriptorSets(
                    command_, VK_PIPELINE_BIND_POINT_COMPUTE, layout, 0, 1, &set, 0, nullptr);
                int parameters[] = {step, order, group, int(preset), int(z)};
                vkCmdPushConstants(command_,
                                   layout,
                                   VK_SHADER_STAGE_COMPUTE_BIT,
                                   0,
                                   sizeof(parameters),
                                   parameters);
                vkCmdDispatch(
                    command_, extent.width / 8, extent.height / 8, std::min(4u, extent.depth - z));
                VkMemoryBarrier barrier{VK_STRUCTURE_TYPE_MEMORY_BARRIER};
                barrier.srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT;
                barrier.dstAccessMask = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_SHADER_WRITE_BIT;
                vkCmdPipelineBarrier(command_,
                                     VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                                     VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                                     0,
                                     1,
                                     &barrier,
                                     0,
                                     nullptr,
                                     0,
                                     nullptr);
                submit();
            }
        };
        for (int group = 0; group < ASTRA_SPECTRAL_GROUPS; ++group) {
            std::cout << "Preset " << preset << ": spectral group " << group + 1 << "/"
                      << ASTRA_SPECTRAL_GROUPS << std::endl;
            dispatch(0, 0, group);
            dispatch(1, 0, group);
            dispatch(2, 1, group);
            for (unsigned order = 2; order <= orders; ++order) {
                dispatch(3, order, group);
                dispatch(4, order, group);
                dispatch(5, order, group);
            }
        }
        dispatch(0, 0, ASTRA_SPECTRAL_GROUPS);
    } catch (...) {
        release();
        throw;
    }
    release();
}

VkDescriptorImageInfo AtmosphereLut::descriptor(unsigned index) const {
    return {sampler_, images_.at(index).view, VK_IMAGE_LAYOUT_GENERAL};
}

std::array<float, 3> AtmosphereLut::irradiance(float height, float source_z) const {
    // Same bilinear mapping as Bruneton's irradiance texture, evaluated once
    // per frame for view-independent exposure rather than in every fragment.
    const float x = std::clamp(source_z * .5f + .5f, 0.f, 1.f) * 63;
    const float y = std::clamp(std::max(.001f, height) / 60.f, 0.f, 1.f) * 15;
    const int ix = int(x), iy = int(y);
    const float fx = x - ix, fy = y - iy;
    std::array<float, 3> result{};
    for (int c = 0; c < 3; ++c) {
        auto value = [&](int dx, int dy) {
            return irradiance_[(std::min(iy + dy, 15) * 64 + std::min(ix + dx, 63)) * 4 + c];
        };
        result[c] = std::max(0.f,
                             std::lerp(std::lerp(value(0, 0), value(1, 0), fx),
                                       std::lerp(value(0, 1), value(1, 1), fx),
                                       fy));
    }
    return result;
}

void AtmosphereLut::cleanup() {
    for (auto& image : images_) {
        if (image.view) {
            vkDestroyImageView(device_, image.view, nullptr);
        }
        if (image.handle) {
            vkDestroyImage(device_, image.handle, nullptr);
        }
        if (image.memory) {
            vkFreeMemory(device_, image.memory, nullptr);
        }
    }
    if (sampler_) {
        vkDestroySampler(device_, sampler_, nullptr);
    }
    if (fence_) {
        vkDestroyFence(device_, fence_, nullptr);
    }
    if (pool_) {
        vkDestroyCommandPool(device_, pool_, nullptr);
    }
}

AtmosphereLut::~AtmosphereLut() {
    cleanup();
}
} // namespace astro
