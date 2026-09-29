use crate::math::{Mat3, PI, Vec3, angle, norm};
use serde::{Deserialize, Serialize};

pub fn disc_overlap(a: f64, b: f64, d: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 || d >= a + b {
        return 0.0;
    }
    if d <= (a - b).abs() {
        return (b * b / (a * a)).min(1.0);
    }
    let (aa, bb, dd) = (a * a, b * b, d * d);
    let x = ((dd + aa - bb) / (2.0 * d * a)).clamp(-1.0, 1.0).acos();
    let y = ((dd + bb - aa) / (2.0 * d * b)).clamp(-1.0, 1.0).acos();
    let triangle = ((-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b))
        .max(0.0)
        .sqrt();
    ((aa * x + bb * y - 0.5 * triangle) / (PI * aa)).clamp(0.0, 1.0)
}
pub fn lunar_sunlight(sun: Vec3, earth: Vec3) -> f64 {
    let solar = (695700.0 / norm(sun)).clamp(0.0, 1.0).asin();
    let terrestrial = (6378.137 * 1.01 / norm(earth)).clamp(0.0, 1.0).asin();
    1.0 - disc_overlap(solar, terrestrial, angle(sun, earth))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LunarSurface {
    pub fixed_from_enu: Mat3,
    pub sun_km: Vec3,
    pub earth_km: Vec3,
    pub solar_visibility: f64,
    pub earthshine_lux: f64,
    pub precise_orientation: bool,
}
impl Default for LunarSurface {
    fn default() -> Self {
        Self {
            fixed_from_enu: Mat3::default(),
            sun_km: Vec3::default(),
            earth_km: Vec3::default(),
            solar_visibility: 1.0,
            earthshine_lux: 0.0,
            precise_orientation: false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::AU_KM;
    #[test]
    fn independent_circle_geometry() {
        assert_eq!(disc_overlap(1.0, 1.0, 3.0), 0.0);
        assert_eq!(disc_overlap(1.0, 2.0, 0.0), 1.0);
        assert!((disc_overlap(2.0, 1.0, 0.0) - 0.25).abs() < 1e-12);
        assert!(
            (disc_overlap(1.0, 1.0, 1.0) - (2.0 * PI / 3.0 - 3.0f64.sqrt() / 2.0) / PI).abs()
                < 1e-12
        );
        assert_eq!(
            lunar_sunlight(
                Vec3 {
                    x: AU_KM,
                    y: 0.0,
                    z: 0.0
                },
                Vec3 {
                    x: 384400.0,
                    y: 0.0,
                    z: 0.0
                }
            ),
            0.0
        );
        assert_eq!(
            lunar_sunlight(
                Vec3 {
                    x: AU_KM,
                    y: 0.0,
                    z: 0.0
                },
                Vec3 {
                    x: 0.0,
                    y: 384400.0,
                    z: 0.0
                }
            ),
            1.0
        );
    }
}
