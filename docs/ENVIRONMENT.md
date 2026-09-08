# Vulkan环境安装与验证

日期：2026-09-04。最初按用户要求安装 Vulkan；用户随后授权实现。本文件保留初次安装结果，应用验证见 [实现记录](IMPLEMENTATION.md)。

## 本机与安装结果

| 项目 | 实际检测结果 |
| --- | --- |
| 系统 | macOS26.4.1，arm64 |
| GPU | Apple M4 Pro |
| 包管理器 | 已有Homebrew，前缀 `/opt/homebrew` |
| Vulkan Loader/headers/tools | 1.4.350.1 |
| Vulkan instance version | 1.4.350 |
| MoltenVK | 1.4.1 |
| GPU报告的Vulkan API版本 | 1.4.334 |
| Khronos validation layers | 1.4.350.1，诊断枚举版本1.4.350 |
| GLSLang | 16.3.0，`glslangValidator`可用 |
| SPIR-V Tools/headers | Homebrew包1.4.350.1；`spirv-val`可用 |

安装的是Homebrew提供的Vulkan开发组件组合，包含MoltenVK、loader、headers、tools、validation layers和GLSL/SPIR-V工具；**不是LunarG完整GUI SDK安装器**。当前未设置`VULKAN_SDK`，也不需要伪造这个路径。项目实现时CMake可使用Homebrew前缀查找组件。

执行的安装命令：

```sh
HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 \
HOMEBREW_NO_INSTALL_CLEANUP=1 \
brew install vulkan-tools vulkan-validationlayers
```

自动安装的依赖：`spirv-headers`、`spirv-tools`、`glslang`、`vulkan-headers`、`vulkan-loader`、`molten-vk`、`vulkan-utility-libraries`。未修改用户shell配置、未改动系统安全设置。

## 已完成的验证

1. `glslangValidator --version`及`spirv-val --version`正常返回。
2. 在可访问Metal GPU的执行环境中运行`vulkaninfo --summary`，识别到Apple M4 Pro和MoltenVK。
3. 显式开启`VK_LAYER_KHRONOS_validation`后重复检查，确保验证层真正可加载。
4. 保存 [vulkaninfo结果](research/vulkaninfo-summary.txt) 和 [stderr记录](research/vulkaninfo-stderr.txt)。最终stderr只有显式添加验证层的提示，没有动态库加载错误或validation error。

实现阶段已运行原生 Vulkan 窗口、HDR 绘制、中文 UI、缩放、鱼眼和截图回读，并启用 Khronos 同步验证。实际日志和截图位于 `artifacts/`。

## 后续开发使用方式

Homebrew版本的验证层manifest使用动态库名，单独设置`VK_LAYER_PATH`可能只能枚举、不能加载。已验证可用的诊断命令是：

```sh
DYLD_LIBRARY_PATH=/opt/homebrew/opt/vulkan-validationlayers/lib \
VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
VK_LAYER_PATH=/opt/homebrew/opt/vulkan-validationlayers/share/vulkan/explicit_layer.d \
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation \
vulkaninfo --summary
```

这些变量仅作用于该命令。显式指定的是真实MoltenVK ICD，不能使用Homebrew提示中用于测试的mock ICD来证明显卡可用。

最初在受限执行沙箱内，工具报告无法访问Metal设备；允许GPU访问后同一安装成功运行。这不意味着硬件不支持Vulkan，也不需要重新安装驱动。验证层第一次显式加载缺动态库路径，补齐上述路径后已验证成功。

未来开发目标仍以运行时支持的特性集为准，不能因为版本字符串为1.4就使用任意扩展。最终`.app`要随包携带必要库和正确rpath，不依赖这些开发机环境变量。

官方依据：[MoltenVK](https://github.com/KhronosGroup/MoltenVK)、[LunarG macOS开发指南](https://vulkan.lunarg.com/doc/view/latest/mac/getting_started.html)、[Homebrew molten-vk](https://formulae.brew.sh/formula/molten-vk)。网页中的最新版本可能已高于当前安装版本；本表记录的是本机工具实际输出。
