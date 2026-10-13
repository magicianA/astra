use crate::math::{PI, RAD, Vec3, cross, dot, norm, unit, wrap};
use serde::{Deserialize, Serialize};

/// Values also identify the projection in the Vulkan scene push constants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[repr(i32)]
#[serde(rename_all = "lowercase")]
pub enum ProjectionKind {
    #[default]
    Perspective = 0,
    Fisheye = 1,
    Stereographic = 2,
}

pub const DEFAULT_CAMERA_FOV: f64 = 60.0 * RAD;
pub const fn maximum_fov(kind: ProjectionKind) -> f64 {
    match kind {
        ProjectionKind::Perspective => 150.0 * RAD,
        _ => 180.0 * RAD,
    }
}
pub const fn maximum_zoom_fov(kind: ProjectionKind) -> f64 {
    match kind {
        ProjectionKind::Perspective => DEFAULT_CAMERA_FOV,
        _ => maximum_fov(kind),
    }
}
pub fn zoom_fov(fov: f64, kind: ProjectionKind, steps: f64) -> f64 {
    (fov * (-steps * 0.12).exp()).clamp(RAD, maximum_zoom_fov(kind))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub azimuth: f64,
    pub elevation: f64,
    pub roll: f64,
    pub fov: f64,
    pub width: f64,
    pub height: f64,
    pub projection: ProjectionKind,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            azimuth: PI,
            elevation: 35.0 * RAD,
            roll: 0.0,
            fov: DEFAULT_CAMERA_FOV,
            width: 1280.0,
            height: 800.0,
            projection: ProjectionKind::Perspective,
        }
    }
}
impl Camera {
    pub fn forward(&self) -> Vec3 {
        Vec3 {
            x: self.azimuth.sin() * self.elevation.cos(),
            y: self.azimuth.cos() * self.elevation.cos(),
            z: self.elevation.sin(),
        }
    }
    pub fn right(&self) -> Vec3 {
        let horizontal = Vec3 {
            x: self.azimuth.cos(),
            y: -self.azimuth.sin(),
            z: 0.0,
        };
        horizontal * self.roll.cos() + cross(horizontal, self.forward()) * self.roll.sin()
    }
    pub fn up(&self) -> Vec3 {
        cross(self.right(), self.forward())
    }
    pub fn pixels_per_radian(&self) -> f64 {
        let field = self.fov.min(maximum_fov(self.projection));
        match self.projection {
            ProjectionKind::Fisheye => self.width.min(self.height) / field,
            ProjectionKind::Stereographic => self.height / (4.0 * (field / 4.0).tan()),
            ProjectionKind::Perspective => self.height / (2.0 * (field / 2.0).tan()),
        }
    }
    pub fn prepare(&self) -> Projection {
        Projection {
            right: self.right(),
            up: self.up(),
            forward: self.forward(),
            scale: self.pixels_per_radian(),
            width: self.width,
            height: self.height,
            half_fov: self.fov.min(maximum_fov(self.projection)) / 2.0,
            kind: self.projection,
        }
    }
    pub fn project(&self, direction: Vec3) -> ScreenPoint {
        self.prepare().project(direction, 0.0)
    }
    pub fn unproject(&self, x: f64, y: f64) -> Vec3 {
        let x = (x - self.width / 2.0) / self.pixels_per_radian();
        let y = (self.height / 2.0 - y) / self.pixels_per_radian();
        match self.projection {
            ProjectionKind::Perspective => unit(self.forward() + self.right() * x + self.up() * y),
            ProjectionKind::Stereographic => {
                let r2 = (x * x + y * y) / 4.0;
                (self.forward() * (1.0 - r2) + self.right() * x + self.up() * y) / (1.0 + r2)
            }
            ProjectionKind::Fisheye => {
                let r = x.hypot(y);
                if r < 1e-15 {
                    self.forward()
                } else {
                    self.forward() * r.cos() + (self.right() * x + self.up() * y) * (r.sin() / r)
                }
            }
        }
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= 0.0
            && y >= 0.0
            && x < self.width
            && y < self.height
            && (self.projection != ProjectionKind::Fisheye
                || (x - self.width / 2.0).hypot(y - self.height / 2.0)
                    <= self.width.min(self.height) / 2.0)
    }
    /// Horizon-aligned dragging never introduces roll, even at the poles.
    pub fn pan(&mut self, from_x: f64, from_y: f64, to_x: f64, to_y: f64) {
        let scale = self.pixels_per_radian();
        let angle_at = |offset: f64| match self.projection {
            ProjectionKind::Fisheye => offset,
            ProjectionKind::Stereographic => 2.0 * (offset / 2.0).atan(),
            ProjectionKind::Perspective => offset.atan(),
        };
        let yaw = angle_at((from_x - self.width / 2.0) / scale)
            - angle_at((to_x - self.width / 2.0) / scale);
        let pitch = angle_at((self.height / 2.0 - from_y) / scale)
            - angle_at((self.height / 2.0 - to_y) / scale);
        self.azimuth = wrap(self.azimuth + yaw);
        self.elevation = (self.elevation + pitch).clamp(-PI / 2.0, PI / 2.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Projection {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub scale: f64,
    pub width: f64,
    pub height: f64,
    pub half_fov: f64,
    pub kind: ProjectionKind,
}
impl Projection {
    pub fn project(&self, v: Vec3, angular_radius: f64) -> ScreenPoint {
        let (mut x, mut y, z) = (dot(v, self.right), dot(v, self.up), dot(v, self.forward));
        match self.kind {
            ProjectionKind::Perspective => {
                if z <= 0.0 {
                    return ScreenPoint::default();
                }
                x = x / z * self.scale;
                y = y / z * self.scale;
            }
            ProjectionKind::Stereographic => {
                let denominator = norm(v) + z;
                if denominator <= 1e-12 {
                    return ScreenPoint::default();
                }
                x *= 2.0 * self.scale / denominator;
                y *= 2.0 * self.scale / denominator;
            }
            ProjectionKind::Fisheye => {
                let r = x.hypot(y);
                let theta = r.atan2(z);
                if theta > self.half_fov + angular_radius {
                    return ScreenPoint::default();
                }
                if r > 1e-15 {
                    x = x / r * theta * self.scale;
                    y = y / r * theta * self.scale;
                } else {
                    x = 0.0;
                    y = 0.0;
                }
            }
        }
        x += self.width / 2.0;
        y = self.height / 2.0 - y;
        let theta = ((x - self.width / 2.0).hypot(y - self.height / 2.0) / self.scale).atan();
        let margin =
            30.0 + angular_radius * self.scale / (theta + angular_radius).cos().max(0.01).powi(2);
        ScreenPoint {
            x,
            y,
            visible: x >= -margin
                && y >= -margin
                && x <= self.width + margin
                && y <= self.height + margin,
        }
    }
    pub fn corner_angle(&self) -> f64 {
        let radius = (self.width + 60.0).hypot(self.height + 60.0) / (2.0 * self.scale);
        match self.kind {
            ProjectionKind::Fisheye => self.half_fov,
            ProjectionKind::Stereographic => 2.0 * (radius / 2.0).atan(),
            ProjectionKind::Perspective => radius.atan(),
        }
    }
    pub fn maximum_scale(&self, point: ScreenPoint) -> f64 {
        let radius = (point.x - self.width / 2.0).hypot(point.y - self.height / 2.0) / self.scale;
        match self.kind {
            ProjectionKind::Fisheye if radius >= 1e-12 => self.scale * radius / radius.sin(),
            ProjectionKind::Fisheye => self.scale,
            ProjectionKind::Stereographic => self.scale * (1.0 + radius * radius / 4.0),
            ProjectionKind::Perspective => self.scale * (1.0 + radius * radius),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::angle;
    #[test]
    fn projection_round_trips_and_pole_pan() {
        for kind in [
            ProjectionKind::Perspective,
            ProjectionKind::Fisheye,
            ProjectionKind::Stereographic,
        ] {
            for elevation in [-90.0, -35.0, 0.0, 89.0, 90.0] {
                let mut camera = Camera {
                    projection: kind,
                    elevation: elevation * RAD,
                    fov: 90.0 * RAD,
                    ..Camera::default()
                };
                for (x, y) in [(640.0, 400.0), (600.0, 450.0), (740.0, 300.0)] {
                    let p = camera.project(camera.unproject(x, y));
                    assert!(p.visible && (p.x - x).hypot(p.y - y) < 1e-9);
                }
                camera.pan(640.0, 400.0, 740.0, 750.0);
                assert_eq!(camera.roll, 0.0);
                assert!(camera.right().z.abs() < 1e-12);
                assert!((norm(camera.up()) - 1.0).abs() < 1e-12);
                assert!(camera.elevation.abs() <= PI / 2.0);
            }
            let mut c = Camera {
                projection: kind,
                fov: maximum_zoom_fov(kind),
                ..Camera::default()
            };
            let initial = c;
            for _ in 0..100 {
                c.fov = zoom_fov(c.fov, kind, 1.125);
                c.fov = zoom_fov(c.fov, kind, -1.125);
            }
            assert!((c.fov - initial.fov).abs() < 1e-12);
            assert!(angle(c.unproject(0.0, 0.0), initial.unproject(0.0, 0.0)) < 1e-10);
        }
    }
    #[test]
    fn stereographic_wide_corners_and_local_conformality() {
        let camera = Camera {
            projection: ProjectionKind::Stereographic,
            fov: PI,
            width: 2560.0,
            height: 900.0,
            ..Camera::default()
        };
        let projection = camera.prepare();
        for (x, y) in [(1.0, 1.0), (2559.0, 899.0), (100.0, 400.0)] {
            let ray = camera.unproject(x, y);
            let p = camera.project(ray);
            assert!(p.visible && (p.x - x).hypot(p.y - y) < 1e-8);
            assert!(angle(ray, camera.forward()) < projection.corner_angle());
            let a = unit(cross(ray, camera.up()));
            let b = unit(cross(ray, a));
            let step: f64 = 1e-6;
            let ap = camera.project(ray * step.cos() + a * step.sin());
            let am = camera.project(ray * step.cos() - a * step.sin());
            let bp = camera.project(ray * step.cos() + b * step.sin());
            let bm = camera.project(ray * step.cos() - b * step.sin());
            let (ax, ay, bx, by) = (ap.x - am.x, ap.y - am.y, bp.x - bm.x, bp.y - bm.y);
            assert!((ax.hypot(ay) / bx.hypot(by) - 1.0).abs() < 1e-7);
            assert!(((ax * bx + ay * by) / ax.hypot(ay) / bx.hypot(by)).abs() < 1e-7);
        }
        assert!(!camera.project(camera.forward() * -1.0).visible);
    }
}
