#include "astro/atmosphere.hpp"
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <vector>

namespace {
void check(VkResult r) {
    if (r != VK_SUCCESS) {
        throw std::runtime_error("Vulkan: " + std::to_string(r));
    }
}

VKAPI_ATTR VkBool32 VKAPI_CALL debug(VkDebugUtilsMessageSeverityFlagBitsEXT severity,
                                     VkDebugUtilsMessageTypeFlagsEXT,
                                     const VkDebugUtilsMessengerCallbackDataEXT* data,
                                     void* errors) {
    if (severity & VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT) {
        ++*static_cast<unsigned*>(errors);
        std::cerr << data->pMessage << '\n';
    }
    return VK_FALSE;
}
} // namespace

int main(int argc, char** argv) {
    VkInstance instance{};
    VkDevice device{};
    VkDebugUtilsMessengerEXT messenger{};
    try {
        if (argc < 2) {
            throw std::runtime_error(
                "Usage: astra_atmosphere OUTPUT_DIRECTORY [ORDERS=8] [--validation]");
        }
        const unsigned orders = argc > 2 ? unsigned(std::stoi(argv[2])) : 8;
        const bool validation = argc > 3 && std::string(argv[3]) == "--validation";
        unsigned errors = 0;
        VkApplicationInfo app{VK_STRUCTURE_TYPE_APPLICATION_INFO};
        app.pApplicationName = "Astra atmosphere preparation";
        app.apiVersion = VK_API_VERSION_1_2;
        std::vector<const char*> extensions;
#ifdef __APPLE__
        extensions.push_back("VK_KHR_portability_enumeration");
#endif
        if (validation) {
            extensions.push_back(VK_EXT_DEBUG_UTILS_EXTENSION_NAME);
        }
        const char* layer = "VK_LAYER_KHRONOS_validation";
        VkInstanceCreateInfo ci{VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO};
        ci.pApplicationInfo = &app;
#ifdef __APPLE__
        ci.flags = VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR;
#endif
        ci.enabledExtensionCount = extensions.size();
        ci.ppEnabledExtensionNames = extensions.data();
        ci.enabledLayerCount = validation ? 1 : 0;
        ci.ppEnabledLayerNames = &layer;
        check(vkCreateInstance(&ci, nullptr, &instance));
        if (validation) {
            VkDebugUtilsMessengerCreateInfoEXT info{
                VK_STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT};
            info.messageSeverity = VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT;
            info.messageType = VK_DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT |
                               VK_DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT;
            info.pfnUserCallback = debug;
            info.pUserData = &errors;
            auto create = reinterpret_cast<PFN_vkCreateDebugUtilsMessengerEXT>(
                vkGetInstanceProcAddr(instance, "vkCreateDebugUtilsMessengerEXT"));
            check(create(instance, &info, nullptr, &messenger));
        }
        uint32_t count;
        check(vkEnumeratePhysicalDevices(instance, &count, nullptr));
        std::vector<VkPhysicalDevice> devices(count);
        check(vkEnumeratePhysicalDevices(instance, &count, devices.data()));
        VkPhysicalDevice physical{};
        uint32_t family = 0;
        for (auto candidate : devices) {
            vkGetPhysicalDeviceQueueFamilyProperties(candidate, &count, nullptr);
            std::vector<VkQueueFamilyProperties> families(count);
            vkGetPhysicalDeviceQueueFamilyProperties(candidate, &count, families.data());
            for (uint32_t i = 0; i < count; ++i) {
                if (families[i].queueFlags & VK_QUEUE_COMPUTE_BIT) {
                    physical = candidate;
                    family = i;
                    break;
                }
            }
            if (physical) {
                break;
            }
        }
        if (!physical) {
            throw std::runtime_error("No Vulkan compute device");
        }
        VkPhysicalDeviceProperties properties;
        vkGetPhysicalDeviceProperties(physical, &properties);
        std::cout << properties.deviceName << std::endl;
        if (properties.limits.maxPerStageDescriptorStorageImages < 9) {
            throw std::runtime_error("Atmosphere generator needs nine storage image bindings");
        }
        float priority = 1;
        VkDeviceQueueCreateInfo qi{VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO};
        qi.queueFamilyIndex = family;
        qi.queueCount = 1;
        qi.pQueuePriorities = &priority;
        VkDeviceCreateInfo di{VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO};
        di.queueCreateInfoCount = 1;
        di.pQueueCreateInfos = &qi;
        const char* portability = "VK_KHR_portability_subset";
#ifdef __APPLE__
        di.enabledExtensionCount = 1;
        di.ppEnabledExtensionNames = &portability;
#else
        (void)portability;
#endif
        check(vkCreateDevice(physical, &di, nullptr, &device));
        VkQueue queue;
        vkGetDeviceQueue(device, family, 0, &queue);
        for (unsigned preset = 0; preset < 2; ++preset) {
            astro::AtmosphereLut lut(physical,
                                     device,
                                     queue,
                                     family,
                                     ASTRA_SHADER_DIR,
                                     std::filesystem::path(argv[1]) /
                                         (preset ? "hazy.bin" : "clear.bin"),
                                     preset,
                                     true,
                                     orders);
        }
        vkDestroyDevice(device, nullptr);
        device = {};
        if (messenger) {
            auto destroy = reinterpret_cast<PFN_vkDestroyDebugUtilsMessengerEXT>(
                vkGetInstanceProcAddr(instance, "vkDestroyDebugUtilsMessengerEXT"));
            destroy(instance, messenger, nullptr);
        }
        messenger = {};
        vkDestroyInstance(instance, nullptr);
        instance = {};
        std::cout << "Validation errors: " << errors << '\n';
        return errors ? 1 : 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        if (device) {
            vkDestroyDevice(device, nullptr);
        }
        if (messenger) {
            auto destroy = reinterpret_cast<PFN_vkDestroyDebugUtilsMessengerEXT>(
                vkGetInstanceProcAddr(instance, "vkDestroyDebugUtilsMessengerEXT"));
            destroy(instance, messenger, nullptr);
        }
        if (instance) {
            vkDestroyInstance(instance, nullptr);
        }
        return 1;
    }
}
