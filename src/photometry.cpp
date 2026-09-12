#include "astro/photometry.hpp"
#include "astro/twilight_calibration.hpp"
#include <algorithm>
#include <cmath>

namespace astro::photometry {
double illuminance(double magnitude) {
    return zero_magnitude_lux * pow(10., -.4 * magnitude);
}

double luminance(double magnitude) {
    return luminance_zero_point * pow(10., -.4 * magnitude);
}

double surface_magnitude(double value) {
    return -2.5 * log10(std::max(value, 1e-30) / luminance_zero_point);
}

double solid_angle(double radius) {
    return 4 * pi * pow(sin(radius / 2), 2);
}

double mean_disc_luminance(double flux, double radius) {
    return flux / std::max(1e-20, solid_angle(radius));
}

double lambert_phase(double fraction) {
    const double cosine = std::clamp(2 * fraction - 1, -1., 1.);
    const double alpha = acos(cosine);
    return std::max(0., (sin(alpha) + (pi - alpha) * cosine) / pi);
}

double gaia_visual_magnitude(double g, double bv) {
    // EDR3 G-V versus BP-RP (ESA documentation Table 5.9). The existing pack
    // stores BV = 0.75*(BP-RP)-0.1, clipped at its endpoints; invert that proxy.
    const double c = std::clamp((bv + .1) / .75, -.5, 5.);
    const double g_minus_v = -.02704 + .01424 * c - .2156 * c * c + .01426 * c * c * c;
    return g - g_minus_v;
}

std::array<float, 3> unit_luminance(std::array<float, 3> rgb) {
    const double y = .2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2];
    if (y <= 0) {
        return {1, 1, 1};
    }
    for (auto& c : rgb) {
        c = float(c / y);
    }
    return rgb;
}

double pixel_solid_angle(double x, double y, double scale, int projection) {
    const double r2 = (x * x + y * y) / (scale * scale);
    double jacobian;
    if (projection == 2) {
        jacobian = 1 / pow(1 + r2 / 4, 2);
    } else if (projection == 1) {
        const double r = sqrt(r2);
        jacobian = r > 1e-8 ? sin(r) / r : 1;
    } else {
        jacobian = pow(1 + r2, -1.5);
    }
    return jacobian / (scale * scale);
}

double twilight_reference(double depression) {
    const double x = std::clamp(depression, 5., 15.) - 5;
    return luminance(11.84 + 1.518 * x - .057 * x * x);
}

double pollution_luminance(double level) {
    // User-selected extra zenith luminance, from zero to roughly 17 mag/arcsec².
    // This is a local skyglow scenario, not a geographic light-pollution atlas.
    return night_floor * (pow(101., std::clamp(level, 0., 1.)) - 1);
}

double twilight_gain(double depression) {
    if (depression <= 4 || depression >= 18) {
        return 1;
    }
    const double x = (std::clamp(depression, 5., 15.) - 5) * 40;
    const size_t i = std::min(size_t(x), twilight_log_gain.size() - 2);
    const double gain = std::lerp(twilight_log_gain[i], twilight_log_gain[i + 1], x - i);
    double weight = depression < 5 ? depression - 4 : (18 - depression) / 3;
    weight = std::clamp(weight, 0., 1.);
    weight *= weight * (3 - 2 * weight);
    return exp(gain * weight);
}

double exposure_gain(double sky, double moon) {
    // One display gain for every source. Include the Moon's direct illuminance
    // as well as diffuse light; the same full sky is metered at every camera angle.
    return .12 / (.12 / night_exposure_gain + std::max(0., sky) + std::max(0., moon) / (2 * pi));
}

double view_exposure_gain(std::span<float> samples) {
    if (samples.empty()) {
        return night_exposure_gain;
    }
    auto end = std::remove_if(samples.begin(), samples.end(), [](float value) {
        return !std::isfinite(value) || value < 0;
    });
    const auto count = size_t(end - samples.begin());
    if (!count) {
        return night_exposure_gain;
    }
    std::sort(samples.begin(), end);
    // Trim the brightest 2% so individual stars do not drive exposure. Retain
    // broad highlights (e.g. a resolved Moon) through the 90th–98th percentile
    // mean. Arithmetic luminance avoids overexposing a disc against black space.
    const size_t high = std::max(size_t(1), count * 98 / 100);
    const size_t tail = std::min(high - 1, count * 90 / 100);
    double sum = 0, bright = 0;
    for (size_t i = 0; i < high; ++i) {
        sum += samples[i];
        if (i >= tail) {
            bright += samples[i];
        }
    }
    const double metered = std::max(sum / high, .2 * bright / (high - tail));
    return std::clamp(
        .12 / (.12 / night_exposure_gain + metered), minimum_exposure_gain, night_exposure_gain);
}

double adapt_exposure(double current, double target, double elapsed) {
    const double minimum = minimum_exposure_gain;
    if (!std::isfinite(target) || target <= 0) {
        return current;
    }
    target = std::clamp(target, minimum, night_exposure_gain);
    if (!std::isfinite(current) || current <= 0) {
        return target;
    }
    if (!std::isfinite(elapsed) || elapsed <= 0) {
        return current;
    }
    // Integrate a rate-limited exponential analytically in EV. The transition
    // is frame-rate independent; pausing/minimizing cannot create a large jump.
    const double dt = std::min(elapsed, .1);
    const double delta = log2(target / current), distance = abs(delta);
    const double tau = delta < 0 ? .45 : 1.2, rate = delta < 0 ? 12. : 4.;
    const double linear = std::max(0., (distance - rate * tau) / rate);
    const double remaining = dt <= linear
                                 ? distance - rate * dt
                                 : std::min(distance, rate * tau) * exp(-(dt - linear) / tau);
    return current * exp2(std::copysign(distance - remaining, delta));
}

double lunar_sky_luminance(double flux, double separation, double moon_t, double view_t) {
    // Krisciunas & Schaefer angular scattering law, as used by ING TN 127.
    // I* is illuminance in footcandles, B is in nanoLamberts. Extinction comes
    // from the atmosphere LUT instead of a fixed site extinction coefficient.
    const double rho = std::clamp(separation / rad, 0., 180.);
    const double scattering =
        pow(10., 5.36) * (1.06 + pow(cos(separation), 2)) + pow(10., 6.15 - rho / 40);
    return scattering * flux / 10.76391 * std::clamp(moon_t, 0., 1.) *
           (1 - std::clamp(view_t, 0., 1.)) * 1e-5 / pi;
}
} // namespace astro::photometry
