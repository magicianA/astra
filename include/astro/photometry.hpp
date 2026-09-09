#pragma once
#include "math.hpp"
#include <array>

namespace astro::photometry {
// Photopic/V-band approximation. Irradiance is illuminance in lux; diffuse
// radiance is luminance in cd/m². See docs/PHOTOMETRY.md for reference spectra.
inline constexpr double zero_magnitude_lux = 2.54e-6;
inline constexpr double square_arcsecond_sr = arcsec * arcsec;
inline constexpr double luminance_zero_point = zero_magnitude_lux / square_arcsecond_sr;
inline constexpr double night_floor = 1.5e-4;
inline constexpr double reference_night = 2.475e-4;
inline constexpr std::array<double, 3> solar_rgb = {144809.86689, 129443.61827, 127098.89412};
inline constexpr double solar_lux =
    .2126 * solar_rgb[0] + .7152 * solar_rgb[1] + .0722 * solar_rgb[2];
inline constexpr const char* model_id = "photometric-v1";

double illuminance(double visual_magnitude);
double luminance(double magnitudes_per_square_arcsecond);
double surface_magnitude(double luminance_cd_m2);
double solid_angle(double angular_radius);
double mean_disc_luminance(double illuminance_lux, double angular_radius);
double lambert_phase(double illuminated_fraction);
double gaia_visual_magnitude(double g, double approximate_bv);
std::array<float, 3> unit_luminance(std::array<float, 3> linear_rgb);
double pixel_solid_angle(double x, double y, double scale, int projection);
// Patat et al. (2006), Table 1, V fit including the natural night background.
// The fit is used only in its measured interval, solar depression 5°–15°.
double twilight_reference(double solar_depression_degrees);
double twilight_gain(double solar_depression_degrees);
double pollution_luminance(double level);
double exposure_gain(double mean_sky_luminance, double direct_moon_illuminance);
double lunar_sky_luminance(double illuminance_lux,
                           double separation,
                           double moon_transmission,
                           double view_transmission);
} // namespace astro::photometry
