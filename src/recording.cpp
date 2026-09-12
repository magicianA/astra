#include "astro/sequence.hpp"
#include <SDL3/SDL.h>
#include <cstdlib>
#include <png.h>
#include <stdexcept>

namespace astro {
Recording::Recording(SequenceRequest r, Scenario s) : request(std::move(r)), initial(std::move(s)) {
    validate_sequence(request);
    sequence_scene(initial, request, request.frames - 1);
    request.directory = std::filesystem::absolute(request.directory);
    if (std::filesystem::exists(request.directory) &&
        !std::filesystem::is_empty(request.directory)) {
        throw std::invalid_argument("Export directory must be empty");
    }
    std::filesystem::create_directories(request.directory);
    save_sequence_manifest(request, initial, 0, false);
}

Recording::~Recording() {
    if (!done_) {
        try {
            cancel();
        } catch (...) {
        }
    }
}

std::filesystem::path Recording::frame_path() const {
    char name[64];
    snprintf(name, sizeof(name), "frame-%06d.png", completed);
    return request.directory / name;
}

void Recording::captured() {
    if (done_ || encoder_ || completed >= request.frames) {
        throw std::logic_error("Recording is not capturing");
    }
    if (request.trails) {
        accumulate_trails(trails_, frame_path(), width_, height_);
    } else {
        png_image image{};
        image.version = PNG_IMAGE_VERSION;
        if (!png_image_begin_read_from_file(&image, frame_path().string().c_str())) {
            const auto error = std::string(image.message);
            png_image_free(&image);
            throw std::runtime_error(error);
        }
        const auto w = image.width, h = image.height;
        png_image_free(&image);
        if (completed && (w != width_ || h != height_)) {
            throw std::runtime_error("Window size changed during export");
        }
        width_ = w;
        height_ = h;
    }
    ++completed;
    save_sequence_manifest(request, initial, completed, false);
    if (completed < request.frames) {
        return;
    }
    if (request.trails) {
        write_trails(trails_, width_, height_, request.directory / "trails.png");
    }
    if (!request.video) {
        done_ = true;
        return;
    }
    std::string executable = "ffmpeg";
    if (const auto* configured = std::getenv("ASTRA_FFMPEG")) {
        executable = configured;
    }
#ifdef __APPLE__
    else if (std::filesystem::exists("/opt/homebrew/bin/ffmpeg")) {
        executable = "/opt/homebrew/bin/ffmpeg";
    } else if (std::filesystem::exists("/usr/local/bin/ffmpeg")) {
        executable = "/usr/local/bin/ffmpeg";
    }
#endif
    const std::vector<std::string> arguments{executable,
                                             "-nostdin",
                                             "-n",
                                             "-loglevel",
                                             "error",
                                             "-framerate",
                                             std::to_string(request.fps),
                                             "-start_number",
                                             "0",
                                             "-i",
                                             (request.directory / "frame-%06d.png").string(),
                                             "-frames:v",
                                             std::to_string(request.frames),
                                             "-vf",
                                             "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                                             "-c:v",
                                             "libx264",
                                             "-crf",
                                             "18",
                                             "-pix_fmt",
                                             "yuv420p",
                                             "-movflags",
                                             "+faststart",
                                             (request.directory / "timelapse.mp4").string()};
    std::vector<const char*> args;
    for (const auto& argument : arguments) {
        args.push_back(argument.c_str());
    }
    args.push_back(nullptr);
    encoder_ = SDL_CreateProcess(args.data(), false);
    if (!encoder_) {
        throw std::runtime_error(
            "Cannot start FFmpeg; PNG frames were saved. Install FFmpeg or set ASTRA_FFMPEG.");
    }
}

bool Recording::poll() {
    if (encoder_) {
        int code = 0;
        if (!SDL_WaitProcess(encoder_, false, &code)) {
            return false;
        }
        SDL_DestroyProcess(encoder_);
        encoder_ = nullptr;
        if (code) {
            throw std::runtime_error("FFmpeg encoding failed; PNG frames were saved");
        }
        done_ = true;
    }
    return done_;
}

void Recording::cancel() {
    if (encoder_) {
        SDL_KillProcess(encoder_, true);
        SDL_WaitProcess(encoder_, true, nullptr);
        SDL_DestroyProcess(encoder_);
        encoder_ = nullptr;
    }
    save_sequence_manifest(request, initial, completed, true);
    done_ = true;
}
} // namespace astro
