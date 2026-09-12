#include "astro/render_scene.hpp"
#include "astro/background.hpp"
#include "astro/photometry.hpp"

namespace astro {
Vec3 disc_tangent(Vec3 centre, Vec3 axis, Vec3 forward) {
    return axis -
           (centre + forward) * (dot(axis, centre) / std::max(1e-12, 1 + dot(centre, forward)));
}

RenderScene render_scene(const SkySnapshot& sky,
                         const SkySnapshot& stars,
                         const Scenario& view,
                         const Camera& camera,
                         const Catalog* catalog) {
    RenderScene result;
    result.camera = camera;
    result.horizon = view.horizon;
    result.lunar = sky.lunar;
    result.moon_surface = view.moon_surface;
    result.milky_way = view.milky_way;
    result.extinction = float(view.extinction);
    result.galactic_rotation = galactic_rotation(sky.celestial_to_enu);
    auto& s = sky.scenario;
    result.atmosphere = s.atmosphere;
    result.ground = s.ground;
    result.pollution = float(s.light_pollution);
    result.exposure = float(view.exposure);
    result.atmosphere_preset = view.atmosphere_preset;
    result.auto_exposure = view.auto_exposure;
    result.adaptive_exposure = view.adaptive_exposure;
    result.height_km = float(s.height / 1000.);
    result.moon_phase = float(sky.moon_phase);
    for (auto& o : sky.bodies) {
        if (o.body == 10) {
            result.sun = o.observed;
            result.geometric_sun = o.geometric;
            result.solar_flux = float(sky.solar_visibility / (o.distance_au * o.distance_au));
        }
        if (o.body == 301) {
            result.moon = o.observed;
            result.geometric_moon = o.geometric;
            // One flux drives both the resolved surface and atmospheric moonlight.
            result.lunar_flux = float(o.illuminance_lux / photometry::solar_lux);
        }
    }
    const auto projection = camera.prepare();
    auto add = [&](const Object& o) {
        auto p = projection.project(o.observed, o.angular_radius);
        if (!p.visible) {
            return;
        }
        if (s.ground && !view.horizon.visible(o.observed, o.angular_radius)) {
            return;
        }
        double mag = o.magnitude;
        bool disk = o.body && (o.body == 301 || o.body == 10 ||
                               o.angular_radius * projection.maximum_scale(p) > 1.3);
        if (!disk && mag > s.magnitude) {
            return;
        }
        // Unresolved sources share a compact screen-space point-spread function.
        // Its six-sigma support is independent of magnitude and camera zoom.
        float radius = disk ? float(o.angular_radius * projection.scale) : 3.f;
        // A point source's integrated illuminance is spread over a Gaussian
        // footprint in solid angle. Resolved discs use their mean luminance.
        const double pixel_area = photometry::pixel_solid_angle(p.x - camera.width / 2,
                                                                p.y - camera.height / 2,
                                                                projection.scale,
                                                                int(projection.kind));
        float strength =
            float(disk ? photometry::mean_disc_luminance(o.unocculted_lux, o.angular_radius)
                       : o.illuminance_lux / (2 * pi * .5 * .5 * pixel_area));
        if (!disk) {
            // Fade through the detection limit instead of toggling stars on/off.
            const double limit = s.magnitude;
            const double visibility = std::clamp((limit - mag) / .75, 0., 1.);
            strength *= float(visibility * visibility * (3 - 2 * visibility));
        }
        const auto color = photometry::unit_luminance(o.color);
        const Vec3 right = disc_tangent(o.observed, projection.right, projection.forward);
        const Vec3 up = disc_tangent(o.observed, projection.up, projection.forward);
        const float limb = float(atan2(dot(o.illumination, up), dot(o.illumination, right)));
        result.points.push_back({float(p.x),
                                 float(p.y),
                                 radius,
                                 disk ? (o.body == 10    ? 1.f
                                         : o.body == 301 ? 3.f
                                                         : 2.f)
                                      : 0.f,
                                 color[0],
                                 color[1],
                                 color[2],
                                 strength,
                                 float(o.phase),
                                 limb,
                                 0,
                                 0});
    };
    const double limiting_magnitude = s.magnitude;
    const double refraction_margin = s.atmosphere ? refraction(-rad, s.pressure, s.temperature) : 0;
    const double corner_angle = projection.corner_angle();
    const double minimum_dot = cos(std::min(pi, corner_angle + refraction_margin));
    const double lowest_altitude =
        sin(*std::min_element(view.horizon.degrees.begin(), view.horizon.degrees.end()) * rad -
            refraction_margin);
    for (const auto& source : stars.stars) {
        if (source.magnitude > limiting_magnitude) {
            continue;
        }
        const Vec3 geometric = sky.celestial_to_enu * source.icrs;
        // A cone around the full viewport, enlarged by the maximum refraction,
        // safely rejects off-screen stars before expensive trigonometry.
        if (dot(geometric, projection.forward) < minimum_dot ||
            (s.ground && geometric.z < lowest_altitude)) {
            continue;
        }
        Object star = source;
        star.geometric = geometric;
        star.observed = s.atmosphere ? refract(geometric, s.pressure, s.temperature) : geometric;
        add(star);
    }
    if (view.constellations && catalog) {
        for (const auto& figure : catalog->constellations) {
            for (const auto& segment : figure.segments) {
                const auto a = stars.guide_stars.find(segment[0]),
                           b = stars.guide_stars.find(segment[1]);
                if (a == stars.guide_stars.end() || b == stars.guide_stars.end()) {
                    continue;
                }
                const Vec3 from = observe_star(a->second, sky).observed;
                const Vec3 to = observe_star(b->second, sky).observed;
                const int steps =
                    std::max(1, int(ceil(angle(from, to) / std::min(2 * rad, camera.fov / 4))));
                Vec3 previous = from;
                for (int i = 1; i <= steps; ++i) {
                    const double t = double(i) / steps;
                    const Vec3 next = unit(from * (1 - t) + to * t);
                    const auto p = projection.project(previous), q = projection.project(next);
                    if ((p.visible || q.visible) && dot(previous, projection.forward) > 0 &&
                        dot(next, projection.forward) > 0) {
                        result.points.push_back({float(p.x),
                                                 float(p.y),
                                                 float(q.x),
                                                 4,
                                                 .26f,
                                                 .52f,
                                                 .68f,
                                                 .18f,
                                                 float(q.y),
                                                 0,
                                                 0,
                                                 0});
                    }
                    previous = next;
                }
            }
        }
    }
    auto bodies = sky.bodies;
    std::sort(bodies.begin(), bodies.end(), [](auto& a, auto& b) {
        return a.distance_au > b.distance_au;
    });
    for (auto& o : bodies) {
        add(o);
    }
    return result;
}

} // namespace astro
