#include "astro/sequence.hpp"
#include "astro/events.hpp"
#include "json.hpp"
#include <fstream>
#include <png.h>
#include <stdexcept>

namespace astro {
void validate_sequence(const SequenceRequest& r) {
    if (r.frames < 2 || r.frames > 100000 || r.fps < 1 || r.fps > 120 ||
        !std::isfinite(r.step_seconds) || abs(r.step_seconds) < .001 ||
        abs(r.step_seconds) > 31557600 || r.directory.empty()) {
        throw std::invalid_argument(
            "Invalid sequence: 2..100000 frames, 1..120 fps, finite nonzero time step");
    }
    validate(r.camera_end);
}

Scenario sequence_scene(const Scenario& start, const SequenceRequest& r, int frame) {
    validate_sequence(r);
    if (frame < 0 || frame >= r.frames) {
        throw std::out_of_range("Invalid sequence frame");
    }
    auto s = frame == 0 ? start : scene_at(start, r.step_seconds * frame);
    if (r.camera_path) {
        if (s.projection != r.camera_end.projection) {
            throw std::invalid_argument("Camera path needs matching projections");
        }
        if (frame == 0) {
            return s;
        }
        double t = double(frame) / (r.frames - 1);
        t = t * t * (3 - 2 * t);
        auto angular = [&](double from, double to) {
            return from + std::remainder(to - from, 360.) * t;
        };
        s.azimuth = wrap(angular(start.azimuth, r.camera_end.azimuth) * rad) / rad;
        s.elevation = std::lerp(start.elevation, r.camera_end.elevation, t);
        s.roll = std::remainder(angular(start.roll, r.camera_end.roll), 360.);
        s.fov = exp(std::lerp(log(start.fov), log(r.camera_end.fov), t));
    }
    validate(s);
    return s;
}

void save_sequence_manifest(const SequenceRequest& r,
                            const Scenario& initial,
                            int completed,
                            bool cancelled) {
    nlohmann::json j{
        {"schema_version", 1},
        {"frames", r.frames},
        {"completed_frames", completed},
        {"fps", r.fps},
        {"step_seconds", r.step_seconds},
        {"video", r.video},
        {"trails", r.trails},
        {"cancelled", cancelled},
        {"camera_path", r.camera_path},
        {"initial_scene", "start.json"},
        {"final_camera", "camera-end.json"},
        {"frame_pattern", "frame-%06d.png"},
        {"trails_method", "maximum displayed luminance; preserves RGB; not radiometric exposure"}};
    save_scenario(initial, r.directory / "start.json");
    save_scenario(r.camera_end, r.directory / "camera-end.json");
    std::ofstream file(r.directory / "sequence.json");
    file << j.dump(2) << '\n';
    if (!file) {
        throw std::runtime_error("Cannot save sequence manifest");
    }
}

void accumulate_trails(std::vector<unsigned char>& pixels,
                       const std::filesystem::path& path,
                       uint32_t& width,
                       uint32_t& height) {
    png_image image{};
    image.version = PNG_IMAGE_VERSION;
    if (!png_image_begin_read_from_file(&image, path.string().c_str())) {
        const auto error = std::string(image.message);
        png_image_free(&image);
        throw std::runtime_error(error);
    }
    image.format = PNG_FORMAT_RGBA;
    std::vector<unsigned char> frame(PNG_IMAGE_SIZE(image));
    if (!png_image_finish_read(&image, nullptr, frame.data(), 0, nullptr)) {
        const auto error = std::string(image.message);
        png_image_free(&image);
        throw std::runtime_error(error);
    }
    if (!pixels.empty() && (image.width != width || image.height != height)) {
        png_image_free(&image);
        throw std::runtime_error("Window size changed during export");
    }
    width = image.width;
    height = image.height;
    png_image_free(&image);
    if (pixels.empty()) {
        pixels = std::move(frame);
        return;
    }
    for (size_t i = 0; i < frame.size(); i += 4) {
        auto luminance = [i](const auto& rgb) {
            return .2126 * rgb[i] + .7152 * rgb[i + 1] + .0722 * rgb[i + 2];
        };
        if (luminance(frame) > luminance(pixels)) {
            std::copy_n(frame.data() + i, 4, pixels.data() + i);
        }
    }
}

void write_trails(const std::vector<unsigned char>& pixels,
                  uint32_t width,
                  uint32_t height,
                  const std::filesystem::path& path) {
    png_image image{};
    image.version = PNG_IMAGE_VERSION;
    image.format = PNG_FORMAT_RGBA;
    image.width = width;
    image.height = height;
    if (pixels.size() != size_t(width) * height * 4 ||
        !png_image_write_to_file(&image, path.string().c_str(), 0, pixels.data(), 0, nullptr)) {
        const auto error = std::string(image.message);
        png_image_free(&image);
        throw std::runtime_error("Trail export failed: " + error);
    }
    png_image_free(&image);
}
} // namespace astro
