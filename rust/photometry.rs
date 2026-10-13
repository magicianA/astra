use crate::{
    calibration::TWILIGHT_LOG_GAIN,
    math::{ARCSEC, PI, RAD},
};
pub const ZERO_MAGNITUDE_LUX: f64 = 2.54e-6;
pub const SQUARE_ARCSECOND_SR: f64 = ARCSEC * ARCSEC;
pub const LUMINANCE_ZERO_POINT: f64 = ZERO_MAGNITUDE_LUX / SQUARE_ARCSECOND_SR;
pub const NIGHT_FLOOR: f64 = 1.5e-4;
pub const REFERENCE_NIGHT: f64 = 2.475e-4;
pub const NIGHT_EXPOSURE_GAIN: f64 = 40.0;
pub const MINIMUM_EXPOSURE_GAIN: f64 = NIGHT_EXPOSURE_GAIN / 1099511627776.0;
pub const SOLAR_RGB: [f64; 3] = [144809.86689, 129443.61827, 127098.89412];
pub const SOLAR_LUX: f64 = 0.2126 * SOLAR_RGB[0] + 0.7152 * SOLAR_RGB[1] + 0.0722 * SOLAR_RGB[2];
pub const MODEL_ID: &str = "photometric-v1";

pub fn illuminance(magnitude: f64) -> f64 {
    ZERO_MAGNITUDE_LUX * 10.0f64.powf(-0.4 * magnitude)
}
pub fn luminance(magnitude: f64) -> f64 {
    LUMINANCE_ZERO_POINT * 10.0f64.powf(-0.4 * magnitude)
}
pub fn surface_magnitude(value: f64) -> f64 {
    -2.5 * (value.max(1e-30) / LUMINANCE_ZERO_POINT).log10()
}
pub fn solid_angle(radius: f64) -> f64 {
    4.0 * PI * (radius / 2.0).sin().powi(2)
}
pub fn mean_disc_luminance(flux: f64, radius: f64) -> f64 {
    flux / solid_angle(radius).max(1e-20)
}
pub fn lambert_phase(fraction: f64) -> f64 {
    let cosine = (2.0 * fraction - 1.0).clamp(-1.0, 1.0);
    let alpha = cosine.acos();
    ((alpha.sin() + (PI - alpha) * cosine) / PI).max(0.0)
}
pub fn gaia_visual_magnitude(g: f64, bv: f64) -> f64 {
    let c = ((bv + 0.1) / 0.75).clamp(-0.5, 5.0);
    g - (-0.02704 + 0.01424 * c - 0.2156 * c * c + 0.01426 * c * c * c)
}
pub fn unit_luminance(mut rgb: [f32; 3]) -> [f32; 3] {
    let y = 0.2126 * rgb[0] as f64 + 0.7152 * rgb[1] as f64 + 0.0722 * rgb[2] as f64;
    if y <= 0.0 {
        return [1.0; 3];
    }
    for channel in &mut rgb {
        *channel = (*channel as f64 / y) as f32;
    }
    rgb
}
pub fn pixel_solid_angle(x: f64, y: f64, scale: f64, projection: i32) -> f64 {
    let r2 = (x * x + y * y) / (scale * scale);
    let jacobian = if projection == 2 {
        1.0 / (1.0 + r2 / 4.0).powi(2)
    } else if projection == 1 {
        let r = r2.sqrt();
        if r > 1e-8 { r.sin() / r } else { 1.0 }
    } else {
        (1.0 + r2).powf(-1.5)
    };
    jacobian / (scale * scale)
}
pub fn twilight_reference(depression: f64) -> f64 {
    let x = depression.clamp(5.0, 15.0) - 5.0;
    luminance(11.84 + 1.518 * x - 0.057 * x * x)
}
pub fn pollution_luminance(level: f64) -> f64 {
    NIGHT_FLOOR * (101.0f64.powf(level.clamp(0.0, 1.0)) - 1.0)
}
pub fn twilight_gain(depression: f64) -> f64 {
    if depression <= 4.0 || depression >= 18.0 {
        return 1.0;
    }
    let x = (depression.clamp(5.0, 15.0) - 5.0) * 40.0;
    let i = (x as usize).min(TWILIGHT_LOG_GAIN.len() - 2);
    let gain =
        TWILIGHT_LOG_GAIN[i] + (TWILIGHT_LOG_GAIN[i + 1] - TWILIGHT_LOG_GAIN[i]) * (x - i as f64);
    let mut weight = if depression < 5.0 {
        depression - 4.0
    } else {
        (18.0 - depression) / 3.0
    }
    .clamp(0.0, 1.0);
    weight *= weight * (3.0 - 2.0 * weight);
    (gain * weight).exp()
}
pub fn exposure_gain(sky: f64, moon: f64) -> f64 {
    0.12 / (0.12 / NIGHT_EXPOSURE_GAIN + sky.max(0.0) + moon.max(0.0) / (2.0 * PI))
}
pub fn view_exposure_gain(samples: &mut [f32]) -> f64 {
    let mut count = 0;
    for i in 0..samples.len() {
        if samples[i].is_finite() && samples[i] >= 0.0 {
            samples.swap(i, count);
            count += 1;
        }
    }
    if count == 0 {
        return NIGHT_EXPOSURE_GAIN;
    }
    samples[..count].sort_unstable_by(f32::total_cmp);
    let high = (count * 98 / 100).max(1);
    let tail = (count * 90 / 100).min(high - 1);
    let sum: f64 = samples[..high].iter().map(|&v| v as f64).sum();
    let bright: f64 = samples[tail..high].iter().map(|&v| v as f64).sum();
    let metered = (sum / high as f64).max(0.2 * bright / (high - tail) as f64);
    (0.12 / (0.12 / NIGHT_EXPOSURE_GAIN + metered))
        .clamp(MINIMUM_EXPOSURE_GAIN, NIGHT_EXPOSURE_GAIN)
}
pub fn adapt_exposure(current: f64, target: f64, elapsed: f64) -> f64 {
    if !target.is_finite() || target <= 0.0 {
        return current;
    }
    let target = target.clamp(MINIMUM_EXPOSURE_GAIN, NIGHT_EXPOSURE_GAIN);
    if !current.is_finite() || current <= 0.0 {
        return target;
    }
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return current;
    }
    let dt = elapsed.min(0.1);
    let delta = (target / current).log2();
    let distance = delta.abs();
    let (tau, rate) = if delta < 0.0 {
        (0.45, 12.0)
    } else {
        (1.2, 4.0)
    };
    let linear = ((distance - rate * tau) / rate).max(0.0);
    let remaining = if dt <= linear {
        distance - rate * dt
    } else {
        distance.min(rate * tau) * (-(dt - linear) / tau).exp()
    };
    current * (distance - remaining).copysign(delta).exp2()
}
pub fn lunar_sky_luminance(flux: f64, separation: f64, moon_t: f64, view_t: f64) -> f64 {
    let rho = (separation / RAD).clamp(0.0, 180.0);
    let scattering =
        10.0f64.powf(5.36) * (1.06 + separation.cos().powi(2)) + 10.0f64.powf(6.15 - rho / 40.0);
    scattering * flux / 10.76391 * moon_t.clamp(0.0, 1.0) * (1.0 - view_t.clamp(0.0, 1.0)) * 1e-5
        / PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        camera::{Camera, ProjectionKind},
        math::{cross, norm},
    };
    #[test]
    fn calibrated_flux_and_discs() {
        assert!((illuminance(0.0) / illuminance(5.0) - 100.0).abs() < 1e-10);
        assert!((0.00016..0.00018).contains(&luminance(22.0)));
        assert!((0.00024..0.00026).contains(&luminance(21.6)));
        assert!((0.3..0.32).contains(&illuminance(-12.73)));
        for magnitude in [-26.74, -12.73, -1.0, 6.0, 12.0, 22.0] {
            assert!((surface_magnitude(luminance(magnitude)) - magnitude).abs() < 1e-10);
        }
        // Integrate the projected Lambertian surface independently of the phase function.
        for phase in [0.01, 0.1, 0.5, 0.9, 1.0] {
            let mut sum = 0.0;
            let ca: f64 = 2.0 * phase - 1.0;
            let sa = (1.0 - ca * ca).sqrt();
            let n = 1000;
            for i in 0..n {
                let r = ((i as f64 + 0.5) / n as f64).sqrt();
                let z = (1.0 - r * r).sqrt();
                for j in 0..n {
                    let theta = 2.0 * PI * (j as f64 + 0.5) / n as f64;
                    sum += (r * theta.cos() * sa + z * ca).max(0.0);
                }
            }
            let integral = sum / (n * n) as f64 / ((2.0 / 3.0) * lambert_phase(phase));
            assert!((integral - 1.0).abs() < 0.002, "phase {phase}: {integral}");
        }
        let flux = illuminance(-12.73);
        let radius = 0.26 * RAD;
        assert!(
            (mean_disc_luminance(flux, radius) * solid_angle(radius) / flux - 1.0).abs() < 1e-12
        );
        assert!(
            (mean_disc_luminance(flux * 4.0, radius * 2.0) / mean_disc_luminance(flux, radius)
                - 1.0)
                .abs()
                < 1e-4
        );
        for rgb in [[1.0, 0.1, 0.01], [0.1, 0.5, 1.0]] {
            let rgb = unit_luminance(rgb);
            assert!(
                (0.2126 * rgb[0] as f64 + 0.7152 * rgb[1] as f64 + 0.0722 * rgb[2] as f64 - 1.0)
                    .abs()
                    < 1e-6
            );
        }
        for boundary in [4.0, 5.0, 15.0, 18.0] {
            assert!((twilight_gain(boundary - 1e-7) - twilight_gain(boundary + 1e-7)).abs() < 1e-5);
        }
        assert_eq!(pollution_luminance(0.0), 0.0);
        assert!(pollution_luminance(1.0) > 0.01);
        let moon_t = 10.0f64.powf(-0.4 * 0.15 / (1.0f64 - 0.96 * 0.5).sqrt());
        let view_t = 10.0f64.powf(-0.4 * 0.15);
        let reference_lux = 10.0f64.powf(-0.4 * 3.84) * 10.76391;
        assert!(
            (lunar_sky_luminance(reference_lux, 45.0 * RAD, moon_t, view_t) / 0.0045732016433163
                - 1.0)
                .abs()
                < 1e-10
        );
        assert_eq!(
            lunar_sky_luminance(reference_lux, 45.0 * RAD, 1.0, 1.0),
            0.0
        );
    }
    #[test]
    fn solid_angles_match_inverse_projection() {
        for kind in [
            ProjectionKind::Perspective,
            ProjectionKind::Fisheye,
            ProjectionKind::Stereographic,
        ] {
            for fov in [5.0, 60.0, 120.0] {
                let camera = Camera {
                    fov: fov * RAD,
                    projection: kind,
                    ..Camera::default()
                };
                let projection = camera.prepare();
                for offset in [0.0, 100.0, 250.0] {
                    let x = camera.width / 2.0 + offset;
                    let y = camera.height / 2.0 + offset / 2.0;
                    let a = camera.unproject(x - 0.5, y);
                    let b = camera.unproject(x + 0.5, y);
                    let c = camera.unproject(x, y - 0.5);
                    let d = camera.unproject(x, y + 0.5);
                    let measured = norm(cross(b - a, d - c));
                    let expected =
                        pixel_solid_angle(offset, offset / 2.0, projection.scale, kind as i32);
                    assert!((measured / expected - 1.0).abs() < 1e-4);
                }
            }
        }
    }
    #[test]
    fn metering_and_frame_rate_independent_adaptation() {
        assert!(exposure_gain(NIGHT_FLOOR, 0.2) < exposure_gain(NIGHT_FLOOR, 0.0) / 10.0);
        assert!(exposure_gain(0.0, 0.0) <= NIGHT_EXPOSURE_GAIN);
        assert!(exposure_gain(NIGHT_FLOOR, 0.0) * NIGHT_FLOOR < 0.006);
        let mut samples = vec![NIGHT_FLOOR as f32; 4096];
        let dark = view_exposure_gain(&mut samples);
        assert!(dark * NIGHT_FLOOR < 0.006 && dark <= NIGHT_EXPOSURE_GAIN);
        samples[..40].fill(1e9);
        assert!((view_exposure_gain(&mut samples) / dark - 1.0).abs() < 1e-6);
        samples.fill(0.0);
        samples[..450].fill(3000.0);
        let lunar = view_exposure_gain(&mut samples);
        assert!((0.3..0.7).contains(&(lunar * 3000.0)));
        samples.fill(1000.0);
        let day = view_exposure_gain(&mut samples);
        assert!((day * 1000.0 - 0.12).abs() < 1e-5);
        samples.fill(2000.0);
        assert!((day / view_exposure_gain(&mut samples) - 2.0).abs() < 1e-5);
        samples.fill(0.0);
        samples[..450].fill(1.5e9);
        assert!(view_exposure_gain(&mut samples) * 1.5e9 < 0.7);
        assert_eq!(
            view_exposure_gain(&mut [-1.0, f32::INFINITY, f32::NAN]),
            NIGHT_EXPOSURE_GAIN
        );
        assert_eq!(view_exposure_gain(&mut []), NIGHT_EXPOSURE_GAIN);
        for (first, target) in [(dark, lunar), (lunar, dark)] {
            let mut reference: Option<f64> = None;
            for fps in [30, 60, 144] {
                let mut value = first;
                for _ in 0..3 * fps {
                    let next = adapt_exposure(value, target, 1.0 / fps as f64);
                    assert!(next >= value.min(target) && next <= value.max(target));
                    assert!((next / value).log2().abs() <= 12.0 / fps as f64 + 1e-10);
                    value = next;
                }
                if let Some(reference) = reference {
                    assert!((value / reference).log2().abs() < 1e-9);
                }
                reference = Some(value);
            }
        }
        assert_eq!(adapt_exposure(dark, lunar, 0.0), dark);
        assert!((adapt_exposure(dark, lunar, 60.0) / dark).log2().abs() <= 1.2 + 1e-9);
        assert_eq!(adapt_exposure(0.0, lunar, 0.01), lunar);
        // Verify point-spread flux over subpixel positions and physical pixel densities.
        for dpi in [1.0, 1.5, 2.0, 3.0] {
            for offset in [0.0, 0.1, 0.25, 0.5, 0.75] {
                let variance = 0.25 + 1.0 / (12.0 * dpi * dpi);
                let mut flux = 0.0;
                for y in -12..=12 {
                    for x in -12..=12 {
                        let dx = (x as f64 + offset) / dpi;
                        let dy = (y as f64 + offset) / dpi;
                        if dx * dx + dy * dy <= 9.0 {
                            flux += (-(dx * dx + dy * dy) / (2.0 * variance)).exp()
                                / (2.0 * PI * variance * dpi * dpi);
                        }
                    }
                }
                assert!((flux - 1.0).abs() < 0.006);
            }
        }
    }
}
