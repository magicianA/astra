#include "astro/events.hpp"
#include "astro/sky.hpp"
#include "json.hpp"
#include <algorithm>
#include <fstream>
#include <iomanip>
#include <iostream>
using namespace astro;

int main(int argc, char** argv) {
    try {
        std::filesystem::path data = ASTRA_SOURCE_DATA;
        Scenario s;
        s.scale = TimeScale::UT1;
        std::string output;
        bool geometric = false;
        int twilight = 0;
        bool plan = false;
        std::optional<EventSearch> search;
        Target first{301, {}}, second{10, {}};
        std::optional<uint32_t> hip;
        double days = 35, separation = 3;
        for (int i = 1; i < argc; i++) {
            std::string a = argv[i];
            auto value = [&]() {
                if (i + 1 >= argc) {
                    throw std::invalid_argument("Missing value for " + a);
                }
                return std::string(argv[++i]);
            };
            if (a == "--generate-cio") {
                auto path = value();
                double step = argc > i + 1 ? std::stod(argv[i + 1]) : 4.;
                Orientation::generate(path, step);
                std::cout << "CIO table written: " << path << " step=" << step << " days\n";
                return 0;
            }
            if (a == "--compare-cio") {
                auto first = value();
                auto second = value();
                Orientation x(first), y(second);
                double worst = 0;
                for (int k = -5000; k <= 5000; k++) {
                    double days = k * 365.25 + .3;
                    worst = std::max(worst, std::abs(x.cio(days) - y.cio(days)) / arcsec);
                }
                std::cout << std::setprecision(12) << "Maximum CIO difference: " << worst
                          << " arcsec\n";
                return worst < .05 ? 0 : 2;
            }
            if (a == "--plan") {
                plan = true;
            } else if (a == "--events") {
                const auto kind = value();
                search.emplace();
                if (kind == "solar") {
                    search->kind = EventKind::SolarEclipse;
                } else if (kind == "lunar") {
                    search->kind = EventKind::LunarEclipse;
                } else if (kind == "approach") {
                    search->kind = EventKind::CloseApproach;
                } else if (kind == "occultation") {
                    search->kind = EventKind::Occultation;
                } else {
                    throw std::invalid_argument("Event type: solar|lunar|approach|occultation");
                }
            } else if (a == "--target") {
                first.body = std::stoi(value());
            } else if (a == "--second") {
                second.body = std::stoi(value());
            } else if (a == "--hip") {
                hip = uint32_t(std::stoul(value()));
            } else if (a == "--days") {
                days = std::stod(value());
            } else if (a == "--separation") {
                separation = std::stod(value());
            } else if (a == "--horizon") {
                s.horizon = load_horizon(value());
            } else if (a == "--data") {
                data = value();
            } else if (a == "--date") {
                auto str = value();
                if (sscanf(str.c_str(),
                           "%d-%d-%dT%d:%d:%lf",
                           &s.date.year,
                           &s.date.month,
                           &s.date.day,
                           &s.date.hour,
                           &s.date.minute,
                           &s.date.second) < 3) {
                    throw std::invalid_argument("Date format: -3000-01-01T22:00:00");
                }
            } else if (a == "--site") {
                auto str = value();
                if (sscanf(str.c_str(), "%lf,%lf,%lf", &s.longitude, &s.latitude, &s.height) < 2) {
                    throw std::invalid_argument("Site: longitude,latitude,height");
                }
            } else if (a == "--scale") {
                auto v = value();
                if (v == "TT") {
                    s.scale = TimeScale::TT;
                } else if (v == "TDB") {
                    s.scale = TimeScale::TDB;
                } else if (v == "UTC") {
                    s.scale = TimeScale::UTC;
                } else if (v == "LMT") {
                    s.scale = TimeScale::LocalMean;
                } else if (v == "UT1") {
                    s.scale = TimeScale::UT1;
                } else {
                    throw std::invalid_argument("Unknown time scale");
                }
            } else if (a == "--magnitude") {
                s.magnitude = std::stod(value());
            } else if (a == "--delta-t") {
                s.override_delta_t = true;
                s.custom_delta_t = std::stod(value());
            } else if (a == "--no-atmosphere") {
                s.atmosphere = false;
            } else if (a == "--output") {
                output = value();
            } else if (a == "--geometric") {
                geometric = true;
            } else if (a == "--twilight") {
                const auto event = value();
                if (event == "next-dawn") {
                    twilight = 1;
                } else if (event == "previous-dawn") {
                    twilight = -1;
                } else if (event == "next-dusk") {
                    twilight = 2;
                } else if (event == "previous-dusk") {
                    twilight = -2;
                } else {
                    throw std::invalid_argument("Unknown twilight event");
                }
            } else if (a == "--scenario") {
                s = load_scenario(value());
            } else if (a == "--help") {
                std::cout
                    << "astra_cli --data DIR --date YEAR-MM-DDTHH:MM:SS --site LON,LAT,HEIGHT "
                       "--scale UT1|TT|TDB|UTC|LMT --no-atmosphere --output result.json "
                       "--twilight next-dawn|previous-dawn|next-dusk|previous-dusk\n"
                       "--plan | --events solar|lunar|approach|occultation --days N "
                       "--target NAIF_ID --second NAIF_ID --hip HIP_ID --separation DEGREES "
                       "--horizon FILE\n";
                return 0;
            } else {
                throw std::invalid_argument("Unknown argument: " + a);
            }
        }
        if (int(plan) + int(search.has_value()) + int(twilight != 0) > 1) {
            throw std::invalid_argument("Choose one of plan, events or twilight");
        }
        for (const auto& migration : s.migrations) {
            std::cerr << "Scene migration: " << migration << '\n';
        }
        SkyEngine engine(data);
        if (twilight) {
            auto event = engine.twilight_view(s, abs(twilight) == 1, twilight > 0 ? 1 : -1);
            if (!event) {
                throw std::runtime_error("No twilight event within 370 days or the data range");
            }
            if (output.empty()) {
                throw std::invalid_argument("--twilight requires --output SCENARIO.json");
            }
            save_scenario(*event, output);
            return 0;
        }
        if (plan || search) {
            if (hip) {
                const auto index = engine.catalog.hip_index(*hip);
                if (!index) {
                    throw std::invalid_argument("HIP target is absent from this catalog");
                }
                (plan ? first : second) = {0, index};
            }
            nlohmann::json report{{"date", format_date(s.date)},
                                  {"time_scale", scale_name(s.scale)},
                                  {"site", {s.longitude, s.latitude, s.height}},
                                  {"data_id", engine.catalog.data_id},
                                  {"model", "spherical-shadow-v1"},
                                  {"time_unit", "seconds from date"}};
            if (plan) {
                const auto result = observing_plan(engine, s, first);
                report["samples"] = nlohmann::json::array();
                for (const auto& p : result.samples) {
                    report["samples"].push_back({{"seconds", p.seconds},
                                                 {"altitude_deg", p.altitude / rad},
                                                 {"sun_altitude_deg", p.sun_altitude / rad},
                                                 {"moon_altitude_deg", p.moon_altitude / rad},
                                                 {"moon_separation_deg", p.moon_separation / rad},
                                                 {"sky_cd_m2", p.sky_luminance},
                                                 {"suitable", p.suitable}});
                }
                report["rises"] = result.rises;
                report["sets"] = result.sets;
                report["transit"] = result.transit;
                report["transit_altitude_deg"] = result.transit_altitude / rad;
                report["recommended"] = result.has_recommendation;
                report["best_time"] = result.best_time;
                report["windows"] = nlohmann::json::array();
                for (const auto& w : result.windows) {
                    report["windows"].push_back({w.start, w.end});
                }
            } else {
                search->first = first;
                search->second = second;
                search->days = days;
                search->maximum_separation = separation * rad;
                report["events"] = nlohmann::json::array();
                for (const auto& event : find_events(engine, s, *search)) {
                    nlohmann::json e{{"type", event.type},
                                     {"peak", event.peak},
                                     {"peak_date", format_date(scene_at(s, event.peak).date)},
                                     {"separation_deg", event.separation / rad},
                                     {"obscuration", event.obscuration},
                                     {"visible", event.visible}};
                    for (auto [key, time] : {std::pair{"start", event.start},
                                             {"end", event.end},
                                             {"inner_start", event.inner_start},
                                             {"inner_end", event.inner_end}}) {
                        e[key] = time ? nlohmann::json(*time) : nlohmann::json(nullptr);
                    }
                    report["events"].push_back(e);
                }
            }
            if (output.empty()) {
                std::cout << report.dump(2) << '\n';
            } else {
                std::ofstream out(output);
                out << report.dump(2) << '\n';
                if (!out) {
                    throw std::runtime_error("Cannot write output");
                }
            }
            return 0;
        }
        auto sky = engine.compute(s);
        nlohmann::json j = {{"data_id", sky->data_id},
                            {"date", format_date(s.date)},
                            {"input_scale", scale_name(s.scale)},
                            {"jd_ut1", sky->time.ut1.value()},
                            {"jd_tt", sky->time.tt.value()},
                            {"jd_tdb", sky->time.tdb.value()},
                            {"delta_t_seconds", sky->time.delta_t_seconds},
                            {"quality", sky->time.note},
                            {"catalog_count", sky->catalog_count},
                            {"computed_stars", sky->stars.size()},
                            {"compute_ms", sky->compute_ms},
                            {"solar_visibility", sky->solar_visibility},
                            {"lunar_solar_visibility", sky->lunar.solar_visibility},
                            {"lunar_precise_orientation", sky->lunar.precise_orientation},
                            {"bodies", nlohmann::json::array()},
                            {"bright_stars", nlohmann::json::array()}};
        auto item = [](const Object& o) {
            return nlohmann::json{{"id", std::to_string(o.id)},
                                  {"hip", o.hip},
                                  {"body", o.body},
                                  {"azimuth_deg", o.azimuth() / rad},
                                  {"altitude_deg", o.altitude() / rad},
                                  {"geometric_altitude_deg", asin(o.geometric.z) / rad},
                                  {"ra_icrs_axes_deg", wrap(atan2(o.icrs.y, o.icrs.x)) / rad},
                                  {"dec_icrs_axes_deg", asin(o.icrs.z) / rad},
                                  {"magnitude", o.magnitude},
                                  {"illuminance_above_atmosphere_lux", o.illuminance_lux},
                                  {"angular_radius_deg", o.angular_radius / rad},
                                  {"distance_au", o.distance_au},
                                  {"phase", o.phase},
                                  {"quality", quality_text(o)}};
        };
        std::unique_ptr<Ephemeris> geometric_ephemeris;
        if (geometric) {
            geometric_ephemeris = std::make_unique<Ephemeris>(data);
        }
        for (auto& o : sky->bodies) {
            auto v = item(o);
            v["name"] = body_name(o.body);
            if (geometric) {
                auto st = geometric_ephemeris->state(o.body, sky->time.tdb);
                v["barycentric_km"] = {st.position.x, st.position.y, st.position.z};
            }
            j["bodies"].push_back(v);
        }
        std::sort(sky->stars.begin(), sky->stars.end(), [](auto& a, auto& b) {
            return a.magnitude < b.magnitude;
        });
        for (size_t k = 0; k < std::min(size_t(25), sky->stars.size()); k++) {
            auto v = item(sky->stars[k]);
            v["name"] = engine.catalog.name(engine.catalog.stars[sky->stars[k].catalog_index]);
            j["bright_stars"].push_back(v);
        }
        if (output.empty()) {
            std::cout << j.dump(2) << '\n';
        } else {
            std::ofstream out(output);
            out << j.dump(2) << '\n';
            if (!out) {
                throw std::runtime_error("Cannot write output");
            }
        }
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Astra: " << e.what() << '\n';
        return 1;
    }
}
