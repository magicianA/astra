#pragma once
#include "sky.hpp"
#include <functional>

namespace astro {
struct Target {
    int body = 301;
    std::optional<uint32_t> star;
};

using CancelCheck = std::function<bool()>;
Scenario scene_at(const Scenario&, double seconds);
Object target_object(const SkySnapshot&, Target);

struct PlanSample {
    double seconds{}, altitude{}, sun_altitude{}, moon_altitude{}, moon_separation{},
        sky_luminance{};
    bool suitable = false;
};

struct TimeWindow {
    double start{}, end{};
};

struct ObservationPlan {
    Scenario initial;
    Target target;
    std::vector<PlanSample> samples;
    std::vector<double> rises, sets;
    std::vector<TimeWindow> windows;
    double transit{}, transit_altitude{}, best_time{};
    bool has_recommendation = false;
};

ObservationPlan observing_plan(const SkyEngine&, const Scenario&, Target, CancelCheck = {});
enum class EventKind { SolarEclipse, LunarEclipse, CloseApproach, Occultation };

struct SkyEvent {
    EventKind kind{};
    double peak{}, separation{}, obscuration{};
    std::optional<double> start, end, inner_start, inner_end;
    bool visible = false;
    std::string type;
};

struct EventSearch {
    EventKind kind = EventKind::SolarEclipse;
    Target first{10, {}}, second{301, {}};
    double days = 35, maximum_separation = 3 * rad;
};

std::vector<SkyEvent>
find_events(const SkyEngine&, const Scenario&, const EventSearch&, CancelCheck = {});
} // namespace astro
