#include "astro/events.hpp"
#include "astro/photometry.hpp"
#include <erfa.h>
#include <stdexcept>

namespace astro {
namespace {
void check_cancel(const CancelCheck& cancelled) {
    if (cancelled && cancelled()) {
        throw std::runtime_error("Search cancelled");
    }
}

template <class F> double root(F f, double a, double b) {
    const bool sign = f(a) > 0;
    while (b - a > .1) {
        const double middle = (a + b) / 2;
        if ((f(middle) > 0) == sign) {
            a = middle;
        } else {
            b = middle;
        }
    }
    return (a + b) / 2;
}

template <class F> double minimum(F f, double a, double b) {
    constexpr double ratio = .61803398874989485;
    double c = b - ratio * (b - a), d = a + ratio * (b - a);
    double fc = f(c), fd = f(d);
    while (b - a > .1) {
        if (fc < fd) {
            b = d;
            d = c;
            fd = fc;
            c = b - ratio * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + ratio * (b - a);
            fd = f(d);
        }
    }
    return (a + b) / 2;
}

std::shared_ptr<SkySnapshot> sample(const SkyEngine& engine,
                                    const Scenario& initial,
                                    Target target,
                                    double seconds,
                                    const CancelCheck& cancelled) {
    check_cancel(cancelled);
    return engine.compute(scene_at(initial, seconds),
                          0,
                          nullptr,
                          target.star ? SkyEngine::Scope::FullSky : SkyEngine::Scope::SolarSystem,
                          target.star);
}

double transmission(double altitude) {
    if (altitude <= -2 * rad) {
        return 0;
    }
    const double z = pi / 2 - std::max(0., altitude);
    return pow(10., -.4 * .15 / sqrt(1 - .96 * pow(sin(z), 2)));
}
} // namespace

Scenario scene_at(const Scenario& initial, double seconds) {
    Scenario result = initial;
    if (initial.scale == TimeScale::UTC && !initial.julian) {
        double u1, u2, a1, a2;
        const auto& d = initial.date;
        if (eraDtf2d("UTC", d.year, d.month, d.day, d.hour, d.minute, d.second, &u1, &u2) < 0 ||
            eraUtctai(u1, u2, &a1, &a2) < 0) {
            throw std::invalid_argument("Invalid UTC date");
        }
        eraTaiutc(a1, a2 + seconds / 86400, &u1, &u2);
        int time[4];
        auto& out = result.date;
        eraD2dtf("UTC", 6, u1, u2, &out.year, &out.month, &out.day, time);
        out.hour = time[0];
        out.minute = time[1];
        out.second = time[2] + time[3] * 1e-6;
    } else {
        result.date =
            from_jd(to_jd(initial.date, initial.julian).add_seconds(seconds), initial.julian);
    }
    return result;
}

Object target_object(const SkySnapshot& sky, Target target) {
    if (target.star) {
        for (const auto& object : sky.stars) {
            if (object.catalog_index == *target.star) {
                return object;
            }
        }
    } else {
        for (const auto& object : sky.bodies) {
            if (object.body == target.body) {
                return object;
            }
        }
    }
    throw std::runtime_error("Target unavailable for this epoch");
}

ObservationPlan observing_plan(const SkyEngine& engine,
                               const Scenario& initial,
                               Target target,
                               CancelCheck cancelled) {
    ObservationPlan plan;
    plan.initial = initial;
    plan.target = target;
    double best = -1;
    auto clearance = [&](double t) {
        const auto sky = sample(engine, initial, target, t, cancelled);
        const auto object = target_object(*sky, target);
        return object.altitude() + object.angular_radius -
               initial.horizon.altitude(object.azimuth());
    };
    double previous_clearance = 0;
    for (int i = 0; i <= 288; ++i) {
        const double t = i * 300.;
        const auto sky = sample(engine, initial, target, t, cancelled);
        const auto object = target_object(*sky, target);
        const auto& moon = sky->bodies[1];
        PlanSample point;
        point.seconds = t;
        point.altitude = object.altitude();
        point.sun_altitude = asin(sky->bodies[0].geometric.z);
        point.moon_altitude = moon.altitude();
        point.moon_separation = angle(object.observed, moon.observed);
        point.sky_luminance =
            photometry::night_floor + photometry::pollution_luminance(initial.light_pollution);
        if (initial.atmosphere && initial.horizon.visible(moon.observed) && object.body != 301) {
            point.sky_luminance +=
                photometry::lunar_sky_luminance(moon.illuminance_lux,
                                                point.moon_separation,
                                                transmission(point.moon_altitude),
                                                transmission(point.altitude));
        }
        // A geometric, clear-weather recommendation; no weather forecast or
        // unvalidated naked-eye limiting-magnitude prediction is implied.
        const double darkness = object.body == 10    ? 90 * rad
                                : object.body == 301 ? -6 * rad
                                                     : -18 * rad;
        point.suitable =
            point.altitude >= 20 * rad && point.sun_altitude <= darkness &&
            initial.horizon.visible(object.observed) &&
            (object.body == 10 || object.body == 301 || !initial.horizon.visible(moon.observed) ||
             point.moon_separation >= 20 * rad);
        if (point.suitable) {
            const double score =
                sin(point.altitude) / (1 + point.sky_luminance / photometry::night_floor);
            if (score > best) {
                best = score;
                plan.best_time = t;
                plan.has_recommendation = true;
            }
            if (plan.windows.empty() || plan.windows.back().end < t - 300) {
                plan.windows.push_back({t, t});
            } else {
                plan.windows.back().end = t;
            }
        }
        plan.samples.push_back(point);
        const double current_clearance =
            object.altitude() + object.angular_radius - initial.horizon.altitude(object.azimuth());
        if (i > 0) {
            const double a = previous_clearance, b = current_clearance;
            if (a < 0 && b >= 0) {
                plan.rises.push_back(root(clearance, t - 300, t));
            }
            if (a > 0 && b <= 0) {
                plan.sets.push_back(root(clearance, t - 300, t));
            }
        }
        previous_clearance = current_clearance;
    }
    const auto top =
        std::max_element(plan.samples.begin(), plan.samples.end(), [](auto& a, auto& b) {
            return a.altitude < b.altitude;
        });
    plan.transit = minimum(
        [&](double t) {
            return -target_object(*sample(engine, initial, target, t, cancelled), target)
                        .altitude();
        },
        std::max(0., top->seconds - 300),
        std::min(86400., top->seconds + 300));
    plan.transit_altitude =
        target_object(*sample(engine, initial, target, plan.transit, cancelled), target).altitude();
    return plan;
}

std::vector<SkyEvent> find_events(const SkyEngine& engine,
                                  const Scenario& initial,
                                  const EventSearch& request,
                                  CancelCheck cancelled) {
    if (!std::isfinite(request.days) || request.days <= 0 || request.days > 370 ||
        !std::isfinite(request.maximum_separation) || request.maximum_separation <= 0 ||
        request.maximum_separation > pi) {
        throw std::invalid_argument(
            "Search interval must be 0..370 days and separation 0..180 degrees");
    }
    if (request.first.star && request.second.star) {
        throw std::invalid_argument("Choose at least one solar-system body");
    }
    Target a = request.first, b = request.second;
    if (request.kind == EventKind::SolarEclipse) {
        a = {10, {}};
        b = {301, {}};
    }
    if (request.kind == EventKind::LunarEclipse) {
        a = {301, {}};
        b = {10, {}};
    }
    if (a.body == b.body && a.star == b.star) {
        throw std::invalid_argument("Choose two different targets");
    }
    const Target stellar = a.star ? a : b;

    struct Geometry {
        double separation, outer, inner, obscuration;
        bool visible;
        std::string type;
    };

    auto geometry = [&](double seconds) {
        const auto sky = sample(engine, initial, stellar, seconds, cancelled);
        const auto one = target_object(*sky, a), two = target_object(*sky, b);
        Geometry g{};
        g.separation = angle(one.observed, two.observed);
        g.outer = one.angular_radius + two.angular_radius;
        g.inner = abs(one.angular_radius - two.angular_radius);
        g.obscuration = one.angular_radius > 0
                            ? disc_overlap(one.angular_radius, two.angular_radius, g.separation)
                            : double(g.separation < two.angular_radius);
        g.visible = initial.horizon.visible(one.observed, one.angular_radius);
        g.type = "Close approach";
        if (request.kind == EventKind::SolarEclipse) {
            g.type = g.separation < g.inner
                         ? (two.angular_radius >= one.angular_radius ? "Total solar eclipse"
                                                                     : "Annular solar eclipse")
                         : "Partial solar eclipse";
        } else if (request.kind == EventKind::LunarEclipse) {
            const auto& l = sky->lunar;
            const double distance = norm(l.earth_km);
            const double er = asin(6378.137 * 1.01 / distance);
            const double sr = asin(695700. / norm(l.sun_km));
            const double mr = asin(1737.4 / distance);
            g.separation = angle(l.sun_km, l.earth_km);
            g.outer = er + sr + mr;
            g.inner = er - sr + mr;
            g.obscuration = 1 - l.solar_visibility;
            g.type = g.separation < er - sr - mr ? "Total lunar eclipse"
                     : g.separation < g.inner    ? "Partial lunar eclipse"
                                                 : "Penumbral lunar eclipse";
        } else if (request.kind == EventKind::Occultation) {
            g.type = "Occultation";
            // The named foreground target must actually be closer to the observer.
            if (one.angular_radius == 0 ||
                (one.body && two.body && one.distance_au >= two.distance_au)) {
                g.outer = -1;
            }
            g.obscuration = two.angular_radius > 0
                                ? disc_overlap(two.angular_radius, one.angular_radius, g.separation)
                                : double(g.separation < one.angular_radius);
        }
        return g;
    };
    const double finish = request.days * 86400;
    std::vector<SkyEvent> events;
    // Samples bracket a minimum, not a threshold crossing: grazing eclipses
    // lasting less than a sample interval are refined rather than skipped.
    double left_time = 0, middle_time = std::min(1800., finish);
    auto left = geometry(left_time), middle = geometry(middle_time);
    auto consider = [&](double left_time, double right_time) {
        const double peak = minimum(
            [&](double t) {
                return geometry(t).separation;
            },
            left_time,
            right_time);
        // Endpoint-only minima are clipped ongoing events, not a peak in this interval.
        if (peak < .15 || finish - peak < .15 ||
            (!events.empty() && abs(events.back().peak - peak) < 1)) {
            return;
        }
        const auto g = geometry(peak);
        const bool close = request.kind == EventKind::CloseApproach;
        if (g.separation < (close ? request.maximum_separation : g.outer)) {
            SkyEvent event;
            event.kind = request.kind;
            event.peak = peak;
            event.separation = g.separation;
            event.obscuration = g.obscuration;
            event.visible = g.visible;
            event.type = g.type;
            auto contact = [&](int direction, bool inner) -> std::optional<double> {
                auto value = [&](double t) {
                    const auto v = geometry(t);
                    return v.separation - (inner   ? v.inner
                                           : close ? request.maximum_separation
                                                   : v.outer);
                };
                if (value(peak) >= 0) {
                    return {};
                }
                double previous = peak;
                for (int i = 1; i <= 96; ++i) {
                    const double next = std::clamp(peak + direction * i * 900., 0., finish);
                    if (value(next) >= 0) {
                        return root(value, std::min(previous, next), std::max(previous, next));
                    }
                    if (next == 0 || next == finish) {
                        return {};
                    }
                    previous = next;
                }
                return {};
            };
            event.start = contact(-1, false);
            event.end = contact(1, false);
            if (!close) {
                event.inner_start = contact(-1, true);
                event.inner_end = contact(1, true);
            }
            if (event.start) {
                event.visible |= geometry(*event.start).visible;
            }
            if (event.end) {
                event.visible |= geometry(*event.end).visible;
            }
            events.push_back(event);
        }
    };
    consider(0, middle_time);
    for (double right_time = std::min(middle_time + 1800, finish); middle_time < finish;
         right_time = std::min(middle_time + 1800, finish)) {
        auto right = geometry(right_time);
        if (middle.separation <= left.separation && middle.separation < right.separation) {
            consider(left_time, right_time);
        }
        if (right_time == finish) {
            consider(middle_time, finish);
        }
        if (events.size() >= 256) {
            break;
        }
        left_time = middle_time;
        left = middle;
        middle_time = right_time;
        middle = right;
    }
    return events;
}
} // namespace astro
