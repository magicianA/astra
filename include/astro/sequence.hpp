#pragma once
#include "scenario.hpp"
#include <span>

struct SDL_Process;

namespace astro {
struct SequenceRequest {
    std::filesystem::path directory;
    int frames = 120, fps = 30;
    double step_seconds = 60;
    bool video = true, trails = false, lock_exposure = true, camera_path = false;
    Scenario camera_end;
};

void validate_sequence(const SequenceRequest&);
Scenario sequence_scene(const Scenario& start, const SequenceRequest&, int frame);
void save_sequence_manifest(const SequenceRequest&, const Scenario&, int completed, bool cancelled);
// A photograph-style lighten composite of rendered frames, not a radiometric
// long-exposure integration. Whole pixels are selected to preserve star colour.
void accumulate_trails(std::vector<unsigned char>& pixels,
                       const std::filesystem::path& frame,
                       uint32_t& width,
                       uint32_t& height);
void write_trails(const std::vector<unsigned char>&,
                  uint32_t width,
                  uint32_t height,
                  const std::filesystem::path&);

class Recording {
    SDL_Process* encoder_ = nullptr;
    std::vector<unsigned char> trails_;
    uint32_t width_ = 0, height_ = 0;
    bool done_ = false;

public:
    SequenceRequest request;
    Scenario initial;
    int completed = 0;
    Recording(SequenceRequest, Scenario);
    ~Recording();
    Recording(const Recording&) = delete;
    Recording& operator=(const Recording&) = delete;
    std::filesystem::path frame_path() const;
    void captured();
    bool poll();

    bool encoding() const {
        return encoder_ != nullptr;
    }

    void cancel();
};
} // namespace astro
