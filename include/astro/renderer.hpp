#pragma once
#include "atmosphere.hpp"
#include "camera.hpp"
#include "render_scene.hpp"
#include <SDL3/SDL.h>
#include <filesystem>
#include <memory>
#include <string>
#include <vector>
#include <vulkan/vulkan.h>
struct ImDrawData;

namespace astro {
class Renderer {
    struct Buffer {
        VkBuffer handle{};
        VkDeviceMemory memory{};
        VkDeviceSize size{};
        void* mapped{};
    };

    struct Texture {
        VkImage image{};
        VkDeviceMemory memory{};
        VkImageView view{};
        VkSampler sampler{};
    };

    struct Frame {
        VkCommandPool pool{};
        VkCommandBuffer command{};
        VkFence fence{};
        VkSemaphore acquired{};
        Buffer instances, atmosphere[2], features[2];
        VkDescriptorSet atmosphere_set[2]{}, feature_set[2]{};
        VkImage hdr{};
        VkDeviceMemory hdr_memory{};
        VkImageView hdr_view{};
        VkFramebuffer hdr_framebuffer{};
        VkDescriptorSet tone_set{};
    };

    SDL_Window* window_{};
    VkInstance instance_{};
    VkDebugUtilsMessengerEXT debug_{};
    VkPhysicalDevice physical_{};
    VkDevice device_{};
    VkQueue queue_{};
    uint32_t family_{};
    VkSurfaceKHR surface_{};
    VkSwapchainKHR swapchain_{};
    VkFormat format_{}, hdr_format_ = VK_FORMAT_R32G32B32A32_SFLOAT;
    VkExtent2D extent_{};
    VkRenderPass render_pass_{}, hdr_pass_{};
    VkDescriptorPool descriptors_{};
    VkDescriptorSetLayout tone_set_layout_{};
    VkDescriptorSetLayout atmosphere_set_layout_{};
    VkDescriptorSetLayout feature_set_layout_{};
    Texture moon_albedo_, moon_height_;
    VkBool32 manual_atmosphere_filtering_ = VK_FALSE;
    std::unique_ptr<AtmosphereLut> atmospheres_[2];
    VkSampler sampler_{};
    VkImage background_{};
    VkDeviceMemory background_memory_{};
    VkImageView background_view_{};
    VkSampler background_sampler_{};
    float background_anisotropy_ = 1;
    VkDescriptorSet background_set_{};
    VkPipelineLayout sky_layout_{}, stars_layout_{}, tone_layout_{};
    VkPipeline sky_pipeline_{}, stars_pipeline_{}, tone_pipeline_{};
    std::vector<VkImage> images_;
    std::vector<VkImageView> views_;
    std::vector<VkFramebuffer> framebuffers_;
    std::vector<VkSemaphore> presented_;
    Frame frames_[2];
    uint32_t frame_ = 0;
    bool rebuild_ = false, imgui_ready_ = false;
    std::filesystem::path shaders_;
    std::string gpu_;
    unsigned errors_ = 0;
    float effective_exposure_ = 40;
    uint32_t memory_type(uint32_t, VkMemoryPropertyFlags) const;
    Buffer buffer(VkDeviceSize, VkBufferUsageFlags);
    void release(Buffer&);
    void create_swapchain();
    void destroy_swapchain();
    void create_pipelines();
    void load_texture(const std::filesystem::path&, Texture&, bool srgb);
    void load_background(const std::filesystem::path&);
    void cleanup();
    VkShaderModule shader(const char*);

public:
    Renderer(SDL_Window*,
             const std::filesystem::path& shader_path,
             const std::filesystem::path& background_path,
             bool validation);
    ~Renderer();
    Renderer(const Renderer&) = delete;
    void init_ui();
    bool render(const RenderScene&,
                ImDrawData*,
                const std::filesystem::path& screenshot = {},
                const RenderScene* comparison = nullptr);

    void request_resize() {
        rebuild_ = true;
    }

    const std::string& gpu() const {
        return gpu_;
    }

    unsigned errors() const {
        return errors_;
    }

    float effective_exposure() const {
        return effective_exposure_;
    }

    static VKAPI_ATTR VkBool32 VKAPI_CALL
    debug_callback(VkDebugUtilsMessageSeverityFlagBitsEXT,
                   VkDebugUtilsMessageTypeFlagsEXT,
                   const VkDebugUtilsMessengerCallbackDataEXT*,
                   void*);
};
} // namespace astro
