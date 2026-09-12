#include "astro/camera.hpp"
#include "astro/photometry.hpp"
#include <iostream>
#include <limits>
#include <stdexcept>
#include <vector>

using namespace astro;
using namespace astro::photometry;

namespace {
int checks = 0;

void require(bool condition, const char* message) {
    ++checks;
    if (!condition) {
        throw std::runtime_error(message);
    }
}
} // namespace

int main() {
    try {
        require(abs(illuminance(0) / illuminance(5) - 100) < 1e-10, "Pogson flux ratio");
        require(luminance(22) > .00016 && luminance(22) < .00018, "natural dark sky scale");
        require(luminance(21.6) > .00024 && luminance(21.6) < .00026, "NPS reference sky");
        require(illuminance(-12.73) > .3 && illuminance(-12.73) < .32,
                "full Moon top-of-atmosphere lux");
        for (double magnitude : {-26.74, -12.73, -1., 6., 12., 22.}) {
            require(abs(surface_magnitude(luminance(magnitude)) - magnitude) < 1e-10,
                    "surface magnitude conversion round trip");
        }
        // Independently integrate projected surface profiles over a unit disc.
        // Flux must equal the ephemeris illuminance for every resolved phase.
        for (double phase : {.01, .1, .5, .9, 1.}) {
            double sum = 0;
            const double ca = 2 * phase - 1, sa = sqrt(1 - ca * ca);
            const int n = 1000;
            for (int i = 0; i < n; ++i) {
                const double r = sqrt((i + .5) / n), z = sqrt(1 - r * r);
                for (int j = 0; j < n; ++j) {
                    const double theta = 2 * pi * (j + .5) / n;
                    sum += std::max(0., r * cos(theta) * sa + z * ca);
                }
            }
            const double integral = sum / (n * n) / ((2. / 3.) * lambert_phase(phase));
            require(abs(integral - 1) < .002, "disc-integrated phase normalization");
        }
        const double moon_flux = illuminance(-12.73), radius = .26 * rad;
        require(abs(mean_disc_luminance(moon_flux, radius) * solid_angle(radius) / moon_flux - 1) <
                    1e-12,
                "resolved disc conserves illuminance");
        require(abs(mean_disc_luminance(moon_flux * 4, radius * 2) /
                        mean_disc_luminance(moon_flux, radius) -
                    1) < 1e-4,
                "surface brightness does not depend on distance");
        // Compare analytic pixel solid angles against a spherical quadrilateral
        // from the actual inverse camera projection, including off-axis views.
        for (auto kind : {ProjectionKind::Perspective,
                          ProjectionKind::Fisheye,
                          ProjectionKind::Stereographic}) {
            for (double fov : {5., 60., 120.}) {
                Camera camera;
                camera.fov = fov * rad;
                camera.projection = kind;
                auto projection = camera.prepare();
                for (double offset : {0., 100., 250.}) {
                    const double x = camera.width / 2 + offset, y = camera.height / 2 + offset / 2;
                    Vec3 a = camera.unproject(x - .5, y), b = camera.unproject(x + .5, y);
                    Vec3 c = camera.unproject(x, y - .5), d = camera.unproject(x, y + .5);
                    const double measured = norm(cross(b - a, d - c));
                    const double expected =
                        pixel_solid_angle(offset, offset / 2, projection.scale, int(kind));
                    require(abs(measured / expected - 1) < 1e-4,
                            "projection solid angle matches camera");
                }
            }
        }
        for (auto rgb : {std::array<float, 3>{1, .1f, .01f}, std::array<float, 3>{.1f, .5f, 1}}) {
            rgb = unit_luminance(rgb);
            require(abs(.2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2] - 1) < 1e-6,
                    "colour must not change source luminance");
        }
        for (double boundary : {4., 5., 15., 18.}) {
            require(abs(twilight_gain(boundary - 1e-7) - twilight_gain(boundary + 1e-7)) < 1e-5,
                    "continuous twilight transitions");
        }
        require(exposure_gain(night_floor, .2) < exposure_gain(night_floor, 0) / 10,
                "direct Moon contributes to full-sky adaptation");
        require(exposure_gain(0, 0) <= night_exposure_gain,
                "dark-scene exposure is bounded even without a sky background");
        require(exposure_gain(night_floor, 0) * night_floor < .006,
                "natural night floor stays below middle-grey display exposure");
        std::vector<float> samples(4096, float(night_floor));
        const double dark_gain = view_exposure_gain(samples);
        require(dark_gain * night_floor < .006 && dark_gain <= night_exposure_gain,
                "adaptive exposure retains a dark natural night sky");
        std::fill_n(samples.begin(), 40, 1e9f);
        require(abs(view_exposure_gain(samples) / dark_gain - 1) < 1e-6,
                "isolated bright stars do not change view exposure");
        std::fill(samples.begin(), samples.end(), 0);
        std::fill_n(samples.begin(), 450, 3000.f);
        const double lunar_gain = view_exposure_gain(samples);
        require(lunar_gain * 3000 > .3 && lunar_gain * 3000 < .7,
                "resolved Moon retains detail against an otherwise black view");
        std::fill(samples.begin(), samples.end(), 1000);
        require(abs(view_exposure_gain(samples) * 1000 - .12) < 1e-5,
                "uniform daylight is metered to the exposure key");
        const double day_gain = view_exposure_gain(samples);
        std::fill(samples.begin(), samples.end(), 2000);
        require(abs(day_gain / view_exposure_gain(samples) - 2) < 1e-5,
                "doubling scene luminance reduces exposure by one stop");
        std::fill(samples.begin(), samples.end(), 0);
        std::fill_n(samples.begin(), 450, 1.5e9f);
        require(view_exposure_gain(samples) * 1.5e9 < .7,
                "meter range includes resolved solar surface luminance");
        samples = {
            -1, std::numeric_limits<float>::infinity(), std::numeric_limits<float>::quiet_NaN()};
        require(view_exposure_gain(samples) == night_exposure_gain &&
                    view_exposure_gain({}) == night_exposure_gain,
                "invalid or absent meter samples cannot poison exposure history");
        for (const auto endpoints :
             {std::array{dark_gain, lunar_gain}, std::array{lunar_gain, dark_gain}}) {
            double reference = 0;
            for (int fps : {30, 60, 144}) {
                double value = endpoints[0];
                for (int frame = 0; frame < 3 * fps; ++frame) {
                    const double next = adapt_exposure(value, endpoints[1], 1. / fps);
                    require(next >= std::min(value, endpoints[1]) &&
                                next <= std::max(value, endpoints[1]),
                            "adaptation is monotonic without overshooting its target");
                    require(abs(log2(next / value)) <= 12. / fps + 1e-10,
                            "exposure changes are bounded in EV per second");
                    value = next;
                }
                if (reference) {
                    require(abs(log2(value / reference)) < 1e-9,
                            "adaptation is independent of display frame rate");
                }
                reference = value;
            }
        }
        require(adapt_exposure(dark_gain, lunar_gain, 0) == dark_gain &&
                    abs(log2(adapt_exposure(dark_gain, lunar_gain, 60) / dark_gain)) <= 1.2 + 1e-9,
                "paused exports freeze adaptation and resuming after stalls is bounded");
        require(adapt_exposure(0, lunar_gain, .01) == lunar_gain,
                "first metering result initializes a valid exposure");
        require(pollution_luminance(0) == 0 && pollution_luminance(1) > .01,
                "pollution adds calibrated sky luminance");
        for (double dpi : {1., 1.5, 2., 3.}) {
            for (double offset : {0., .1, .25, .5, .75}) {
                const double variance = .25 + 1 / (12 * dpi * dpi);
                double flux = 0;
                for (int y = -12; y <= 12; ++y) {
                    for (int x = -12; x <= 12; ++x) {
                        const double dx = (x + offset) / dpi, dy = (y + offset) / dpi;
                        if (dx * dx + dy * dy <= 9) {
                            flux += exp(-(dx * dx + dy * dy) / (2 * variance)) /
                                    (2 * pi * variance * dpi * dpi);
                        }
                    }
                }
                require(abs(flux - 1) < .006,
                        "point flux conserved during subpixel motion and DPI changes");
            }
        }
        // ING/Krisciunas-Schaefer example: full Moon, 45-degree elevation,
        // zenith sightline, k_V=0.15. Expected value computed in nL first.
        const double moon_t = pow(10., -.4 * .15 / sqrt(1 - .96 * .5));
        const double view_t = pow(10., -.4 * .15);
        const double reference_lux = pow(10., -.4 * 3.84) * 10.76391;
        require(
            abs(lunar_sky_luminance(reference_lux, 45 * rad, moon_t, view_t) / .0045732016433163 -
                1) < 1e-10,
            "lunar scattering units agree with the ING reference example");
        require(lunar_sky_luminance(reference_lux, 45 * rad, 1, 1) == 0,
                "no atmosphere means no scattered moonlight");
        std::cout << checks << " photometry checks passed\n";
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
