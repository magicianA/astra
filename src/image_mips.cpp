#include "astro/image_mips.hpp"
#include <algorithm>
#include <array>
#include <cmath>
#include <stdexcept>

namespace astro {
ImageMipChain make_srgb_mips(uint32_t width, uint32_t height, std::span<const uint8_t> rgba) {
    if (!width || !height || width > 16384 || height > 8192 ||
        rgba.size() != size_t(width) * height * 4) {
        throw std::invalid_argument("Invalid RGBA8 background dimensions");
    }
    static const auto linear = [] {
        std::array<double, 256> values{};
        for (size_t i = 0; i < values.size(); ++i) {
            const double v = double(i) / 255;
            values[i] = v <= .04045 ? v / 12.92 : pow((v + .055) / 1.055, 2.4);
        }
        return values;
    }();
    ImageMipChain chain;
    size_t bytes = 0;
    for (;;) {
        chain.levels.push_back({width, height, bytes});
        bytes += size_t(width) * height * 4;
        if (width == 1 && height == 1) {
            break;
        }
        width = std::max(1u, width / 2);
        height = std::max(1u, height / 2);
    }
    chain.pixels.resize(bytes);
    std::copy(rgba.begin(), rgba.end(), chain.pixels.begin());
    for (size_t level = 1; level < chain.levels.size(); ++level) {
        const auto& source = chain.levels[level - 1];
        const auto& target = chain.levels[level];
        const double scale_x = double(source.width) / target.width;
        const double scale_y = double(source.height) / target.height;
        for (uint32_t y = 0; y < target.height; ++y) {
            const double top = y * scale_y, bottom = (y + 1) * scale_y;
            for (uint32_t x = 0; x < target.width; ++x) {
                const double left = x * scale_x, right = (x + 1) * scale_x;
                std::array<double, 4> sum{};
                // Area weights include every edge texel when a dimension is odd.
                const auto end_y = std::min(source.height, uint32_t(ceil(bottom)));
                const auto end_x = std::min(source.width, uint32_t(ceil(right)));
                for (uint32_t sy = uint32_t(top); sy < end_y; ++sy) {
                    const double wy = std::min(bottom, sy + 1.) - std::max(top, double(sy));
                    for (uint32_t sx = uint32_t(left); sx < end_x; ++sx) {
                        const double weight =
                            wy * (std::min(right, sx + 1.) - std::max(left, double(sx)));
                        const size_t offset = source.offset + (size_t(sy) * source.width + sx) * 4;
                        for (size_t c = 0; c < 3; ++c) {
                            sum[c] += linear[chain.pixels[offset + c]] * weight;
                        }
                        sum[3] += double(chain.pixels[offset + 3]) / 255 * weight;
                    }
                }
                const size_t offset = target.offset + (size_t(y) * target.width + x) * 4;
                for (size_t c = 0; c < 4; ++c) {
                    double value = sum[c] / (scale_x * scale_y);
                    if (c < 3) {
                        value =
                            value <= .0031308 ? value * 12.92 : 1.055 * pow(value, 1 / 2.4) - .055;
                    }
                    chain.pixels[offset + c] = uint8_t(std::clamp(lround(value * 255), 0l, 255l));
                }
            }
        }
    }
    return chain;
}
} // namespace astro
