#pragma once

#include <array>
#include <filesystem>
#include <vulkan/vulkan.h>

namespace astro {
// Immutable, device-local transport tables. The generator owns temporary images;
// the application only uploads the four completed tables for each preset.
class AtmosphereLut {
    struct Image {
        VkImage handle{};
        VkDeviceMemory memory{};
        VkImageView view{};
        VkExtent3D extent{};
    };

    VkPhysicalDevice physical_{};
    VkDevice device_{};
    VkQueue queue_{};
    VkCommandPool pool_{};
    VkCommandBuffer command_{};
    VkFence fence_{};
    VkSampler sampler_{};
    VkBool32 manual_filtering_ = VK_FALSE;
    std::array<Image, 9> images_{};
    std::array<float, 64 * 16 * 4> irradiance_{};
    uint32_t memory_type(uint32_t bits, VkMemoryPropertyFlags flags) const;
    void create_image(unsigned index);
    void begin();
    void submit();
    void cleanup();
    void transfer(const std::filesystem::path&, bool write, unsigned preset, unsigned orders);
    void generate(const std::filesystem::path& shaders, unsigned preset, unsigned orders);

public:
    AtmosphereLut(VkPhysicalDevice,
                  VkDevice,
                  VkQueue,
                  uint32_t family,
                  const std::filesystem::path& shaders,
                  const std::filesystem::path& cache,
                  unsigned preset,
                  bool prepare = false,
                  unsigned orders = 8);
    ~AtmosphereLut();
    AtmosphereLut(const AtmosphereLut&) = delete;
    VkDescriptorImageInfo descriptor(unsigned index) const;
    std::array<float, 3> irradiance(float height_km, float source_z) const;
};
} // namespace astro
