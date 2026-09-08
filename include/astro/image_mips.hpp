#pragma once
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

namespace astro {
struct ImageLevel {
    uint32_t width, height;
    size_t offset;
};

struct ImageMipChain {
    std::vector<uint8_t> pixels;
    std::vector<ImageLevel> levels;
};

// RGBA8 input/output, with RGB averaged in linear light and alpha averaged linearly.
// CPU generation keeps filtered backgrounds available without GPU blit requirements.
ImageMipChain make_srgb_mips(uint32_t width, uint32_t height, std::span<const uint8_t> rgba);
} // namespace astro
