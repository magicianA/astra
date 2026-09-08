#include "astro/image_mips.hpp"
#include <algorithm>
#include <array>
#include <cmath>
#include <iostream>
#include <stdexcept>

using namespace astro;

namespace {
int checks = 0;

void require(bool condition, const char* message) {
    ++checks;
    if (!condition) {
        throw std::runtime_error(message);
    }
}
} // namespace

int main() {
    try {
        // Black/white downsampling must conserve light, not average gamma-encoded bytes.
        const std::array<uint8_t, 16> checker{
            0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0};
        const auto mixed = make_srgb_mips(2, 2, checker);
        require(mixed.levels.size() == 2 && mixed.pixels.size() == 20, "complete mip chain");
        require(std::equal(checker.begin(), checker.end(), mixed.pixels.begin()),
                "native-resolution texels remain unchanged");
        require(mixed.pixels[16] == 188 && mixed.pixels[17] == 188 && mixed.pixels[18] == 188,
                "half the incident light encodes as sRGB 188, not 128");
        require(mixed.pixels[19] == 128, "alpha is averaged linearly");

        // The last column of a non-power-of-two map cannot be discarded.
        const std::array<uint8_t, 12> edge{0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0, 255};
        const auto odd = make_srgb_mips(3, 1, edge);
        require(odd.pixels[12] == 156 && odd.pixels[13] == 0 && odd.pixels[14] == 0,
                "odd-dimension border contributes its full area");
        require(odd.pixels[15] == 255, "opaque map stays opaque");

        for (auto size :
             {std::array{1u, 1u}, {1u, 17u}, {17u, 1u}, {125u, 63u}, {250u, 125u}, {16384u, 2u}}) {
            std::vector<uint8_t> pixels(size_t(size[0]) * size[1] * 4);
            constexpr std::array<uint8_t, 4> color{47, 119, 201, 255};
            for (size_t i = 0; i < pixels.size(); ++i) {
                pixels[i] = color[i % 4];
            }
            const auto chain = make_srgb_mips(size[0], size[1], pixels);
            size_t end = 0;
            for (const auto& level : chain.levels) {
                require(level.offset == end && level.offset % 4 == 0,
                        "Vulkan copy regions are contiguous and texel aligned");
                end += size_t(level.width) * level.height * 4;
                require(end <= chain.pixels.size(), "mip region stays in its buffer");
                for (size_t i = level.offset; i < end; ++i) {
                    require(chain.pixels[i] == color[i % 4],
                            "uniform sky brightness stays constant at every LOD");
                }
            }
            require(chain.levels.back().width == 1 && chain.levels.back().height == 1,
                    "rectangular mip chain reaches one texel");
        }
        for (auto size : {std::array{0u, 1u}, {16385u, 1u}, {1u, 8193u}, {2u, 2u}}) {
            bool rejected = false;
            try {
                make_srgb_mips(size[0], size[1], {});
            } catch (const std::invalid_argument&) {
                rejected = true;
            }
            require(rejected, "invalid dimensions or truncated pixels are rejected");
        }
        std::cout << checks << " image mip checks passed\n";
    } catch (const std::exception& error) {
        std::cerr << "Image mip test failed: " << error.what() << '\n';
        return 1;
    }
}
