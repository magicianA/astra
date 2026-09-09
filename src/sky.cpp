#include "astro/sky.hpp"
#include "astro/photometry.hpp"
#include <SpiceUsr.h>
#include <chrono>
#include <erfa.h>
#include <mutex>
#include <stdexcept>
#include <unordered_map>

namespace astro {
static std::mutex spice_mutex;
static std::unordered_map<std::string, unsigned> kernel_references;

// CSPICE's kernel pool is process-global, even for distinct SkyEngine objects.
// Calls and reference counts share the same lock so destroying one engine
// cannot unload a kernel still in use by another.
static void release_kernel(const std::string& path) {
    auto reference = kernel_references.find(path);
    if (reference != kernel_references.end() && --reference->second == 0) {
        unload_c(path.c_str());
        kernel_references.erase(reference);
    }
}

static void spice_check() {
    if (failed_c()) {
        char msg[2048];
        getmsg_c("LONG", sizeof(msg), msg);
        reset_c();
        throw std::runtime_error(std::string("SPICE: ") + msg);
    }
}

Ephemeris::Ephemeris(const std::filesystem::path& root) {
    std::lock_guard guard(spice_mutex);
    erract_c("SET", 0, (SpiceChar*)"RETURN");
    errprt_c("SET", 0, (SpiceChar*)"NONE");
    try {
        for (const char* f : {"de441_part-1.bsp", "de441_part-2.bsp"}) {
            auto p = root / "kernels" / f;
            if (!std::filesystem::exists(p)) {
                throw std::runtime_error(std::string("Missing DE441 kernel: ") + p.string() +
                                         ". Run scripts/bootstrap.py --kernels");
            }
            const auto path = std::filesystem::canonical(p).string();
            if (!kernel_references.contains(path)) {
                furnsh_c(path.c_str());
                spice_check();
            }
            ++kernel_references[path];
            kernels_.push_back(path);
        }
    } catch (...) {
        for (auto& p : kernels_) {
            release_kernel(p);
        }
        throw;
    }
}

Ephemeris::~Ephemeris() {
    std::lock_guard guard(spice_mutex);
    for (auto& p : kernels_) {
        release_kernel(p);
    }
}

State Ephemeris::state(int body, JulianDate jd) const {
    std::lock_guard guard(spice_mutex);
    double pv[6], lt;
    double et = (jd.day - 2451545.) * 86400. + jd.fraction * 86400.;
    spkez_c(body, et, "J2000", "NONE", 0, pv, &lt);
    spice_check();
    return {{pv[0], pv[1], pv[2]}, {pv[3], pv[4], pv[5]}};
}

SkyEngine::SkyEngine(const std::filesystem::path& p)
    : eop_(p / "time/finals2000A.all"), orientation_(p / "time/cio.bin"), ephemeris_(p),
      catalog(p) {}

double refraction(double alt, double pressure, double temperature) {
    if (pressure <= 0 || alt < -2 * rad || alt > 89.9 * rad) {
        return 0;
    }
    double d = std::max(-1., alt / rad);
    double r = 1.02 / tan((d + 10.3 / (d + 5.11)) * rad) / 60 * rad * (pressure / 1010) *
               (283 / (273 + temperature));
    if (alt < -rad) {
        r *= std::clamp((alt / rad + 2), 0., 1.);
    }
    return std::max(0., r);
}

Vec3 refract(Vec3 v, double pressure, double temp) {
    const double altitude = asin(std::clamp(v.z, -1., 1.));
    const double correction = refraction(altitude, pressure, temp);
    if (correction == 0) {
        return v;
    }
    const double horizontal = hypot(v.x, v.y);
    if (altitude + correction >= pi / 2 || horizontal < 1e-15) {
        return {0, 0, 1};
    }
    // Rotate in the vertical plane without converting to and from azimuth.
    const double sine = sin(correction), cosine = cos(correction);
    const double ratio = cosine - v.z * sine / horizontal;
    return {v.x * ratio, v.y * ratio, v.z * cosine + horizontal * sine};
}

static Vec3 vec(const double* p) {
    return {p[0], p[1], p[2]};
}

static void copy(Vec3 v, double* out) {
    for (int i = 0; i < 3; i++) {
        out[i] = v[i];
    }
}

std::string body_name(int b) {
    switch (b) {
    case 10:
        return "太阳 Sun";
    case 301:
        return "月球 Moon";
    case 199:
        return "水星 Mercury";
    case 299:
        return "金星 Venus";
    case 4:
        return "火星 Mars";
    case 5:
        return "木星 Jupiter";
    case 6:
        return "土星 Saturn";
    case 7:
        return "天王星 Uranus";
    case 8:
        return "海王星 Neptune";
    default:
        return "?";
    }
}

std::string quality_text(const Object& o) {
    if (o.body) {
        return o.body >= 4 && o.body <= 8 ? "DE441 · 行星系统质心近似" : "DE441 · 站心视位置";
    }
    std::string s = o.flags & Gaia ? "Gaia DR3" : "Hipparcos-2";
    if (o.flags & RVUnknown) {
        s += " · 径向速度未知";
    }
    if (o.flags & DistanceUnknown) {
        s += " · 距离未知";
    }
    if (o.flags & PMUnknown) {
        s += " · 固定方向近似";
    }
    if (o.flags & Multiple) {
        s += " · 多星/光心近似";
    }
    if (o.flags & LowQuality) {
        s += " · 低质量解";
    }
    if (o.flags & PropagationWarning) {
        s += " · 运动传播近似警告";
    }
    if (o.flags & PositionMatch) {
        s += " · HIP 位置匹配";
    }
    return s;
}

std::shared_ptr<SkySnapshot> SkyEngine::compute(const Scenario& s,
                                                uint64_t generation,
                                                const std::atomic<uint64_t>* latest,
                                                Scope scope) const {
    auto start = std::chrono::steady_clock::now();
    validate(s);
    auto out = std::make_shared<SkySnapshot>();
    out->scenario = s;
    out->generation = generation;
    out->data_id = catalog.data_id;
    out->catalog_count = catalog.stars.size();
    auto& t = out->time;
    t = make_time(
        s.date, s.scale, s.longitude, eop_, s.override_delta_t, s.custom_delta_t, s.julian);
    Mat3 c2t = orientation_.terrestrial(t), enu = enu_basis(s.longitude * rad, s.latitude * rad);
    out->celestial_to_enu = enu * c2t;
    double station[3];
    if (eraGd2gc(1, s.longitude * rad, s.latitude * rad, s.height, station)) {
        throw std::runtime_error("Invalid WGS84 station");
    }
    Vec3 site = (c2t.transpose() * vec(station)) / 1000.;
    // Differentiate the complete terrestrial transform: includes polar motion,
    // precession/nutation and Earth rotation, not only a guessed z-axis spin.
    TimeContext before = t, after = t;
    before.ut1 = t.ut1.add_seconds(-.5);
    before.tt = t.tt.add_seconds(-.5);
    after.ut1 = t.ut1.add_seconds(.5);
    after.tt = t.tt.add_seconds(.5);
    Vec3 site_velocity = ((orientation_.terrestrial(after).transpose() * vec(station)) -
                          (orientation_.terrestrial(before).transpose() * vec(station))) /
                         1000.;
    auto earth = ephemeris_.state(399, t.tdb), sun = ephemeris_.state(10, t.tdb);
    Vec3 observer = earth.position + site;
    double pv[2][3], ebpv[2][3], ehp[3];
    copy(site * 1000, pv[0]);
    copy(site_velocity * 1000, pv[1]);
    copy(earth.position / au_km, ebpv[0]);
    copy(earth.velocity * (86400 / au_km), ebpv[1]);
    copy((earth.position - sun.position) / au_km, ehp);
    eraASTROM astrom{};
    eraApcs(t.tdb.day, t.tdb.fraction, pv, ebpv, ehp, &astrom);
    astrom.pmt = 0;

    struct Body {
        int id;
        double radius, m0;
        std::array<float, 3> color;
    };

    const Body definitions[] = {{10, 695700, -26.74, {1.f, .92f, .65f}},
                                {301, 1737.4, -12.73, {.8f, .86f, .93f}},
                                {199, 2439.7, -.42, {.75f, .7f, .62f}},
                                {299, 6051.8, -4.4, {1.f, .89f, .69f}},
                                {4, 3389.5, -1.52, {1.f, .48f, .25f}},
                                {5, 69911, -9.4, {1.f, .85f, .65f}},
                                {6, 58232, -8.88, {.91f, .78f, .52f}},
                                {7, 25362, -7.19, {.46f, .87f, .89f}},
                                {8, 24622, -6.87, {.35f, .52f, 1.f}}};
    for (auto b : definitions) {
        State target = ephemeris_.state(b.id, t.tdb);
        double light = 0;
        for (int i = 0; i < 8; i++) {
            double next = norm(target.position - observer) / c_kms;
            target = ephemeris_.state(b.id, t.tdb.add_seconds(-next));
            if (abs(next - light) < 1e-8) {
                light = next;
                break;
            }
            light = next;
        }
        Vec3 r = target.position - observer;
        double distance = norm(r);
        Vec3 direction = unit(r);
        double p[3], apparent[3];
        copy(direction, p);
        if (b.id != 10) {
            double q[3], e[3], deflected[3];
            copy(unit(target.position - sun.position), q);
            copy(unit(observer - sun.position), e);
            eraLd(1, p, q, e, norm(observer - sun.position) / au_km, 1e-6, deflected);
            copy(vec(deflected), p);
        }
        eraAb(p, astrom.v, astrom.em, astrom.bm1, apparent);
        Object o;
        o.id = uint64_t(b.id);
        o.body = b.id;
        o.icrs = vec(apparent);
        o.geometric = unit(out->celestial_to_enu * o.icrs);
        o.observed = s.atmosphere ? refract(o.geometric, s.pressure, s.temperature) : o.geometric;
        o.distance_au = distance / au_km;
        o.angular_radius = asin(std::clamp(b.radius / distance, 0., 1.));
        o.color = b.color;
        Vec3 illumination = unit(sun.position - target.position);
        double ca = std::clamp(dot(illumination, unit(observer - target.position)), -1., 1.);
        double phaseangle = acos(ca);
        o.phase = b.id == 10 ? 1 : (1 + ca) / 2;
        if (b.id == 10) {
            o.magnitude = -26.74 + 5 * log10(o.distance_au);
        } else if (b.id == 301) {
            o.magnitude = -12.73 + 1.49 * phaseangle + .0431 * pow(phaseangle, 4) +
                          5 * log10(distance / 384400.) +
                          5 * log10(norm(target.position - sun.position) / au_km);
        } else {
            o.magnitude =
                b.m0 + 5 * log10(o.distance_au * norm(target.position - sun.position) / au_km) -
                2.5 * log10(std::max(.001, (sin(phaseangle) + (pi - phaseangle) * ca) / pi));
        }
        // The shader uses the actual projected Sun direction for the bright limb.
        o.illuminance_lux = b.id == 10 ? photometry::solar_lux / (o.distance_au * o.distance_au)
                                       : photometry::illuminance(o.magnitude);
        if (b.id == 10 || b.id == 301) {
            for (int i = 0; i < 3; ++i) {
                o.color[i] = float(photometry::solar_rgb[i] / photometry::solar_lux);
            }
        }
        Vec3 lightenu = out->celestial_to_enu * illumination;
        Vec3 east = unit(cross({0, 0, 1}, o.geometric));
        Vec3 north = cross(o.geometric, east);
        o.bright_limb_angle = atan2(dot(lightenu, north), dot(lightenu, east));
        out->bodies.push_back(o);
        if (b.id == 10) {
            out->sun_altitude = o.altitude();
        }
        if (b.id == 301) {
            out->moon_altitude = o.altitude();
            out->moon_phase = o.phase;
        }
    }
    if (scope == Scope::SolarSystem) {
        out->compute_ms =
            std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - start)
                .count();
        return out;
    }
    out->stars.reserve(30000);
    double tcb1, tcb2;
    eraTdbtcb(t.tdb.day, t.tdb.fraction, &tcb1, &tcb2);
    for (size_t i = 0; i < catalog.stars.size(); i++) {
        if ((i & 4095) == 0 && latest && latest->load() != generation) {
            return {};
        }
        const auto& star = catalog.stars[i];
        double days = (star.flags & Gaia) ? (tcb1 - 2451545.) + tcb2 : t.tdb.since_j2000();
        double years = 2000 + days / 365.25 - star.epoch;
        double plx = star.flags & DistanceUnknown ? 0 : star.parallax;
        double mag = star.magnitude;
        // Conservative nominal lower bound: radial displacement alone can only
        // underestimate total distance. Do not cull by original sky tile/position.
        double lower = plx > 0 ? abs(1 + star.rv * years * 31557600. * plx * arcsec / au_km) : 1;
        if (mag + 5 * log10(std::max(1e-12, lower)) > s.magnitude + .4) {
            continue;
        }
        double ra2 = star.ra, de2 = star.dec, pmr = 0, pmd = 0, px2 = plx, rv2 = star.rv;
        uint32_t quality = star.flags;
        if (plx > 0) {
            double epoch = 2451545. + (star.epoch - 2000) * 365.25;
            int status = eraPmsafe(star.ra,
                                   star.dec,
                                   star.pmra / std::max(1e-12, cos(star.dec)),
                                   star.pmdec,
                                   plx,
                                   star.rv,
                                   2451545.,
                                   epoch - 2451545.,
                                   2451545.,
                                   days,
                                   &ra2,
                                   &de2,
                                   &pmr,
                                   &pmd,
                                   &px2,
                                   &rv2);
            if (status < 0) {
                continue;
            }
            if (status > 0) {
                quality |= PropagationWarning;
            }
            if (px2 > 0) {
                mag -= 5 * log10(px2 / plx);
            }
        } else {
            Vec3 u = sphere(star.ra, star.dec), ea{-sin(star.ra), cos(star.ra), 0},
                 ed{-sin(star.dec) * cos(star.ra), -sin(star.dec) * sin(star.ra), cos(star.dec)};
            u = unit(u + (ea * star.pmra + ed * star.pmdec) * years);
            ra2 = atan2(u.y, u.x);
            de2 = asin(u.z);
        }
        if (mag > s.magnitude + .4) {
            continue;
        }
        double rai, dei;
        eraAtciq(ra2, de2, 0, 0, px2, 0, &astrom, &rai, &dei);
        Object o;
        o.id = star.id;
        o.hip = star.hip;
        o.catalog_index = uint32_t(i);
        o.flags = quality;
        o.icrs = sphere(rai, dei);
        o.geometric = unit(out->celestial_to_enu * o.icrs);
        o.observed = s.atmosphere ? refract(o.geometric, s.pressure, s.temperature) : o.geometric;
        o.magnitude = mag;
        o.distance_au = px2 > 0 ? 1 / (px2 * arcsec) : 0;
        o.color = star_color(star.bv);
        const double visual =
            star.flags & Gaia ? photometry::gaia_visual_magnitude(mag, star.bv) : mag;
        o.illuminance_lux = photometry::illuminance(visual);
        for (auto& channel : o.color) {
            channel =
                channel <= .04045f ? channel / 12.92f : powf((channel + .055f) / 1.055f, 2.4f);
        }
        o.formal_error_arcsec = hypot(hypot(star.ra_error, star.dec_error),
                                      abs(years) * hypot(star.pmra_error, star.pmdec_error)) /
                                1000.;
        if (o.magnitude <= 2.7 && !((o.flags & Gaia) && !o.hip)) {
            out->label_stars.push_back(out->stars.size());
        }
        out->stars.push_back(o);
    }
    out->compute_ms =
        std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - start).count();
    return out;
}

Object observe_star(const Object& star, const SkySnapshot& frame) {
    Object result = star;
    result.geometric = frame.celestial_to_enu * star.icrs;
    const auto& scene = frame.scenario;
    result.observed = scene.atmosphere
                          ? refract(result.geometric, scene.pressure, scene.temperature)
                          : result.geometric;
    return result;
}

std::optional<Scenario> SkyEngine::next_moon_view(const Scenario& initial,
                                                  uint64_t generation,
                                                  const std::atomic<uint64_t>* latest) const {
    validate(initial);
    const auto start = to_jd(initial.date, initial.julian);
    // Sample the user's clock every half hour. This is a viewing suggestion,
    // not a rise/set event solver; preserve their calendar and time scale.
    for (int step = 0; step <= 35 * 48; ++step) {
        if (latest && latest->load() != generation) {
            return std::nullopt;
        }
        Scenario candidate = initial;
        candidate.date = from_jd(start.add_seconds(step * 1800.), initial.julian);
        if (candidate.date.year >= 7000 ||
            (candidate.scale == TimeScale::UTC && candidate.date.year >= 2027)) {
            break;
        }
        auto sky = compute(candidate, generation, latest, Scope::SolarSystem);
        if (sky->moon_altitude >= 8 * rad && sky->sun_altitude <= -6 * rad &&
            sky->moon_phase >= .02) {
            const auto moon = std::find_if(sky->bodies.begin(), sky->bodies.end(), [](auto& body) {
                return body.body == 301;
            });
            candidate.azimuth = moon->azimuth() / rad;
            candidate.elevation = moon->altitude() / rad;
            candidate.roll = 0;
            candidate.fov = 6;
            candidate.projection = ProjectionKind::Perspective;
            return candidate;
        }
    }
    return std::nullopt;
}

std::optional<Scenario> SkyEngine::twilight_view(const Scenario& initial,
                                                 bool dawn,
                                                 int direction,
                                                 uint64_t generation,
                                                 const std::atomic<uint64_t>* latest) const {
    validate(initial);
    if (direction != -1 && direction != 1) {
        throw std::invalid_argument("Twilight search direction must be -1 or +1");
    }
    const auto start = to_jd(initial.date, initial.julian);
    Scenario candidate = initial;
    auto cancelled = [&] {
        return latest && latest->load() != generation;
    };
    auto sample = [&](double seconds) -> std::optional<Object> {
        if (cancelled()) {
            return std::nullopt;
        }
        candidate.date = from_jd(start.add_seconds(seconds), initial.julian);
        if (candidate.date.year < -3000 || candidate.date.year >= 7000 ||
            (candidate.scale == TimeScale::UTC &&
             (candidate.date.year < 1973 || candidate.date.year >= 2027))) {
            return std::nullopt;
        }
        auto sky = compute(candidate, generation, latest, Scope::SolarSystem);
        if (!sky) {
            return std::nullopt;
        }
        for (const auto& body : sky->bodies) {
            if (body.body == 10) {
                return body;
            }
        }
        return std::nullopt;
    };
    constexpr double threshold = -.10452846326765347; // sin(-6 degrees)
    double previous_time = direction * 2.;            // avoid returning the current event again
    auto previous = sample(previous_time);
    if (!previous) {
        return std::nullopt;
    }
    for (int step = 1; step <= 370 * 48; ++step) {
        double current_time = direction * (2. + step * 1800.);
        auto current = sample(current_time);
        if (!current) {
            return std::nullopt;
        }
        double left_value = (direction > 0 ? previous : current)->geometric.z - threshold;
        double right_value = (direction > 0 ? current : previous)->geometric.z - threshold;
        bool crossing =
            dawn ? left_value < 0 && right_value >= 0 : left_value > 0 && right_value <= 0;
        double left = std::min(previous_time, current_time);
        double right = std::max(previous_time, current_time);
        if (!crossing && left_value * right_value > 0 &&
            std::min(std::abs(left_value), std::abs(right_value)) < .005) {
            // Near a grazing high-latitude event, both crossings may fall
            // inside one coarse interval. Refine its extremum before deciding
            // there is no event. Solar altitude is unimodal over half an hour.
            const bool maximize = left_value < 0;
            double lo = left, hi = right;
            for (int iteration = 0; iteration < 24; ++iteration) {
                const double t1 = lo + (hi - lo) / 3;
                const double t2 = hi - (hi - lo) / 3;
                auto sun1 = sample(t1), sun2 = sample(t2);
                if (!sun1 || !sun2) {
                    return std::nullopt;
                }
                if ((sun1->geometric.z < sun2->geometric.z) == maximize) {
                    lo = t1;
                } else {
                    hi = t2;
                }
            }
            const double middle = (lo + hi) / 2;
            auto sun = sample(middle);
            if (!sun) {
                return std::nullopt;
            }
            const double value = sun->geometric.z - threshold;
            if (maximize ? value > 0 : value < 0) {
                crossing = true;
                if (dawn == maximize) {
                    right = middle;
                } else {
                    left = middle;
                }
            }
        }
        if (crossing) {
            while (right - left > .1) {
                const double middle = (left + right) / 2;
                auto sun = sample(middle);
                if (!sun) {
                    return std::nullopt;
                }
                if ((sun->geometric.z < threshold) == dawn) {
                    left = middle;
                } else {
                    right = middle;
                }
            }
            auto sun = sample((left + right) / 2);
            if (!sun) {
                return std::nullopt;
            }
            candidate.azimuth = sun->azimuth() / rad;
            candidate.elevation = 8;
            candidate.roll = 0;
            candidate.fov = default_camera_fov / rad;
            candidate.projection = ProjectionKind::Perspective;
            candidate.atmosphere = candidate.ground = true;
            return candidate;
        }
        previous = current;
        previous_time = current_time;
    }
    return std::nullopt;
}
} // namespace astro
