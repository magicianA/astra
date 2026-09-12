#include "astro/sequence.hpp"
#include "json.hpp"
#include <SDL3/SDL.h>
#include <chrono>
#include <fstream>
#include <iostream>
#include <stdexcept>

using namespace astro;

namespace {
void require(bool value, const char* message) {
    if (!value) {
        throw std::runtime_error(message);
    }
}

template <class F> void rejects(F f, const char* message) {
    bool rejected = false;
    try {
        f();
    } catch (const std::exception&) {
        rejected = true;
    }
    require(rejected, message);
}
} // namespace

int main() {
    const auto directory =
        std::filesystem::temp_directory_path() /
        ("astra-sequence-" +
         std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    try {
        SDL_Init(0);
        Scenario scene;
        scene.date = {2026, 1, 1, 0, 0, 0};
        scene.scale = TimeScale::UT1;
        scene.azimuth = 350;
        scene.fov = 60;
        SequenceRequest request;
        request.frames = 3;
        request.step_seconds = 3600;
        request.video = false;
        request.directory = directory / "frames";
        request.camera_path = true;
        request.camera_end = scene;
        request.camera_end.azimuth = 10;
        request.camera_end.fov = 15;
        request.camera_end.elevation = 50;
        const auto start = sequence_scene(scene, request, 0);
        const auto middle = sequence_scene(scene, request, 1);
        const auto end = sequence_scene(scene, request, 2);
        require(start == scene, "Sequence starts at exact initial scene");
        require(middle.date.hour == 1 && end.date.hour == 2, "Clock advances by fixed steps");
        require(abs(middle.azimuth) < 1e-8 && abs(middle.fov - 30) < 1e-8,
                "Shortest azimuth and logarithmic field of view");
        require(abs(end.azimuth - 10) < 1e-8 && abs(end.fov - 15) < 1e-8 &&
                    abs(end.elevation - 50) < 1e-8,
                "Camera reaches specified endpoint");
        request.camera_end.projection = ProjectionKind::Fisheye;
        rejects(
            [&] {
                sequence_scene(scene, request, 0);
            },
            "Mismatched projections rejected");
        request.camera_end = scene;
        request.camera_path = false;
        request.trails = true;
        request.frames = 2;
        const std::vector<unsigned char> red{180, 20, 10, 255, 0, 0, 0, 255};
        const std::vector<unsigned char> blue{0, 0, 255, 255, 30, 40, 60, 255};
        {
            Recording recording(request, scene);
            write_trails(red, 2, 1, recording.frame_path());
            recording.captured();
            write_trails(blue, 2, 1, recording.frame_path());
            recording.captured();
            require(recording.poll() && recording.completed == 2,
                    "Recording completes exact frame count");
        }
        std::vector<unsigned char> composite;
        uint32_t width = 0, height = 0;
        accumulate_trails(composite, request.directory / "trails.png", width, height);
        require(composite == std::vector<unsigned char>({180, 20, 10, 255, 30, 40, 60, 255}),
                "Lighten composite preserves whole pixel colour");
        rejects(
            [&] {
                Recording recording(request, scene);
            },
            "Existing output never overwritten");
        request.directory = directory / "cancelled";
        {
            Recording recording(request, scene);
            write_trails(red, 2, 1, recording.frame_path());
            recording.captured();
            recording.cancel();
        }
        auto manifest = nlohmann::json::parse(std::ifstream(request.directory / "sequence.json"));
        require(manifest["cancelled"] == true && manifest["completed_frames"] == 1 &&
                    std::filesystem::exists(request.directory / "frame-000000.png"),
                "Cancelled recording preserves completed frames");
        request.directory = directory / "resize";
        {
            Recording recording(request, scene);
            write_trails(red, 2, 1, recording.frame_path());
            recording.captured();
            write_trails(red, 1, 2, recording.frame_path());
            rejects(
                [&] {
                    recording.captured();
                },
                "Mid-export dimensions cannot change");
        }
        SDL_SetEnvironmentVariable(
            SDL_GetEnvironment(), "ASTRA_FFMPEG", "/missing-astra-test-ffmpeg", true);
        // SDL reads the real process environment for std::getenv as well.
#ifdef _WIN32
        _putenv_s("ASTRA_FFMPEG", "Z:\\missing-astra-test-ffmpeg.exe");
#else
        setenv("ASTRA_FFMPEG", "/missing-astra-test-ffmpeg", 1);
#endif
        request.directory = directory / "encoder-failure";
        request.video = true;
        {
            Recording recording(request, scene);
            write_trails(red, 2, 1, recording.frame_path());
            recording.captured();
            write_trails(blue, 2, 1, recording.frame_path());
            rejects(
                [&] {
                    recording.captured();
                },
                "Unavailable encoder reports failure");
            require(recording.completed == 2 &&
                        std::filesystem::exists(recording.frame_path().parent_path() /
                                                "frame-000001.png"),
                    "Encoder failure preserves all PNG frames");
        }
        std::filesystem::remove_all(directory);
        SDL_Quit();
        std::cout << "Sequence timing, camera, compositing, cancellation and error checks passed\n";
    } catch (const std::exception& e) {
        std::filesystem::remove_all(directory);
        SDL_Quit();
        std::cerr << e.what() << '\n';
        return 1;
    }
}
