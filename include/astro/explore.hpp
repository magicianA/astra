#pragma once
#include "events.hpp"
#include "sequence.hpp"
#include <future>
#include <mutex>

namespace astro {
struct ExploreResult {
    Scenario scene;
    EventSearch search;
    std::optional<ObservationPlan> plan;
    std::vector<SkyEvent> events;
};

struct Exploration {
    int tab = 0, figure = 0, target_body = 1, event_kind = 0, first_body = 1, second_body = 0;
    double event_days = 35, separation_degrees = 3;
    bool use_selected = false, use_selected_event = false;
    std::future<ExploreResult> job;
    std::shared_ptr<std::atomic<bool>> cancelled = std::make_shared<std::atomic<bool>>(false);
    std::optional<ExploreResult> result;
    SequenceRequest sequence;
    bool recording = false, begin_recording = false, cancel_recording = false;
    int completed_frames = 0;
    char horizon_path[1024]{}, output_path[1024]{};
    std::mutex dialog_mutex;
    std::string dialog_horizon, dialog_output;
    bool dialog_open = false;

    ~Exploration() {
        cancelled->store(true);
    }
};
struct UiState;
struct UiFrame;
struct UiActions;
void draw_exploration(UiState&, const UiFrame&, UiActions&);
} // namespace astro
