use crate::background::galactic_rotation;
use crate::camera::{Camera, Projection};
use crate::catalog::Catalog;
use crate::eclipse::LunarSurface;
use crate::horizon::Horizon;
use crate::math::{PI, RAD, Vec3, angle, dot, unit};
use crate::photometry;
use crate::scenario::Scenario;
use crate::sky::{Object, SkySnapshot, observe_star, refract, refraction};

/// Matches the original Vulkan instance buffer: three aligned vec4 attributes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "gpu", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct StarInstance {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub kind: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub brightness: f32,
    pub phase: f32,
    pub limb: f32,
    pub pad0: f32,
    pub pad1: f32,
}
const _: () = assert!(std::mem::size_of::<StarInstance>() == 48);
#[derive(Clone, Debug)]
pub struct RenderScene {
    pub camera: Camera,
    pub sun: Vec3,
    pub moon: Vec3,
    pub geometric_sun: Vec3,
    pub geometric_moon: Vec3,
    pub solar_flux: f32,
    pub lunar_flux: f32,
    pub height_km: f32,
    pub atmosphere_preset: i32,
    pub auto_exposure: bool,
    pub adaptive_exposure: bool,
    pub atmosphere: bool,
    pub ground: bool,
    pub milky_way: bool,
    pub extinction: f32,
    pub galactic_rotation: [f32; 4],
    pub pollution: f32,
    pub moon_phase: f32,
    pub exposure: f32,
    pub points: Vec<StarInstance>,
    pub horizon: Horizon,
    pub lunar: LunarSurface,
    pub moon_surface: bool,
}
impl Default for RenderScene {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            sun: Vec3::new(0.0, 0.0, -1.0),
            moon: Vec3::new(0.0, 0.0, -1.0),
            geometric_sun: Vec3::new(0.0, 0.0, -1.0),
            geometric_moon: Vec3::new(0.0, 0.0, -1.0),
            solar_flux: 1.0,
            lunar_flux: 0.0,
            height_km: 0.0,
            atmosphere_preset: 0,
            auto_exposure: true,
            adaptive_exposure: false,
            atmosphere: true,
            ground: true,
            milky_way: false,
            extinction: 0.2,
            galactic_rotation: [0.0, 0.0, 0.0, 1.0],
            pollution: 0.05,
            moon_phase: 0.0,
            exposure: 1.0,
            points: Vec::new(),
            horizon: Horizon::default(),
            lunar: LunarSurface::default(),
            moon_surface: true,
        }
    }
}
pub fn disc_tangent(centre: Vec3, axis: Vec3, forward: Vec3) -> Vec3 {
    axis - (centre + forward) * (dot(axis, centre) / (1.0 + dot(centre, forward)).max(1e-12))
}
fn add_object(result: &mut RenderScene, o: &Object, view: &Scenario, projection: &Projection) {
    let p = projection.project(o.observed, o.angular_radius);
    if !p.visible || (view.ground && !view.horizon.visible(o.observed, o.angular_radius)) {
        return;
    }
    let disk = o.body != 0
        && (o.body == 301 || o.body == 10 || o.angular_radius * projection.maximum_scale(p) > 1.3);
    if !disk && o.magnitude > view.magnitude {
        return;
    }
    let radius = if disk {
        (o.angular_radius * projection.scale) as f32
    } else {
        3.0
    };
    let pixel_area = photometry::pixel_solid_angle(
        p.x - result.camera.width / 2.0,
        p.y - result.camera.height / 2.0,
        projection.scale,
        projection.kind as i32,
    );
    let mut strength = if disk {
        photometry::mean_disc_luminance(o.unocculted_lux, o.angular_radius)
    } else {
        o.illuminance_lux / (2.0 * PI * 0.5 * 0.5 * pixel_area)
    } as f32;
    if !disk {
        let visibility = ((view.magnitude - o.magnitude) / 0.75).clamp(0.0, 1.0);
        strength *= (visibility * visibility * (3.0 - 2.0 * visibility)) as f32;
    }
    let color = photometry::unit_luminance(o.color);
    let right = disc_tangent(o.observed, projection.right, projection.forward);
    let up = disc_tangent(o.observed, projection.up, projection.forward);
    result.points.push(StarInstance {
        x: p.x as f32,
        y: p.y as f32,
        radius,
        kind: if disk {
            match o.body {
                10 => 1.0,
                301 => 3.0,
                _ => 2.0,
            }
        } else {
            0.0
        },
        r: color[0],
        g: color[1],
        b: color[2],
        brightness: strength,
        phase: o.phase as f32,
        limb: dot(o.illumination, up).atan2(dot(o.illumination, right)) as f32,
        pad0: 0.0,
        pad1: 0.0,
    });
}
pub fn render_scene(
    sky: &SkySnapshot,
    stars: &SkySnapshot,
    view: &Scenario,
    camera: &Camera,
    catalog: Option<&Catalog>,
) -> RenderScene {
    let s = &sky.scenario;
    let mut result = RenderScene {
        camera: *camera,
        horizon: view.horizon.clone(),
        lunar: sky.lunar,
        moon_surface: view.moon_surface,
        milky_way: view.milky_way,
        extinction: view.extinction as f32,
        galactic_rotation: galactic_rotation(&sky.celestial_to_enu),
        atmosphere: s.atmosphere,
        ground: view.ground,
        pollution: view.light_pollution as f32,
        exposure: view.exposure as f32,
        atmosphere_preset: view.atmosphere_preset,
        auto_exposure: view.auto_exposure,
        adaptive_exposure: view.adaptive_exposure,
        height_km: (s.height / 1000.0) as f32,
        moon_phase: sky.moon_phase as f32,
        ..RenderScene::default()
    };
    for o in &sky.bodies {
        if o.body == 10 {
            result.sun = o.observed;
            result.geometric_sun = o.geometric;
            result.solar_flux = (sky.solar_visibility / (o.distance_au * o.distance_au)) as f32;
        }
        if o.body == 301 {
            result.moon = o.observed;
            result.geometric_moon = o.geometric;
            result.lunar_flux = (o.illuminance_lux / photometry::SOLAR_LUX) as f32;
        }
    }
    let projection = camera.prepare();
    let refraction_margin = if s.atmosphere {
        refraction(-RAD, s.pressure, s.temperature)
    } else {
        0.0
    };
    let minimum_dot = (projection.corner_angle() + refraction_margin)
        .min(PI)
        .cos();
    let lowest_altitude = (view
        .horizon
        .degrees
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min) as f64
        * RAD
        - refraction_margin)
        .sin();
    for source in &stars.stars {
        if source.magnitude > view.magnitude {
            continue;
        }
        let geometric = sky.celestial_to_enu * source.icrs;
        if dot(geometric, projection.forward) < minimum_dot
            || (view.ground && geometric.z < lowest_altitude)
        {
            continue;
        }
        let mut star = *source;
        star.geometric = geometric;
        star.observed = if s.atmosphere {
            refract(geometric, s.pressure, s.temperature)
        } else {
            geometric
        };
        add_object(&mut result, &star, view, &projection);
    }
    if view.constellations {
        if let Some(catalog) = catalog {
            for figure in &catalog.constellations {
                for segment in &figure.segments {
                    let (Some(a), Some(b)) = (
                        stars.guide_stars.get(&segment[0]),
                        stars.guide_stars.get(&segment[1]),
                    ) else {
                        continue;
                    };
                    let from = observe_star(a, sky).observed;
                    let to = observe_star(b, sky).observed;
                    let steps =
                        1.max((angle(from, to) / (2.0 * RAD).min(camera.fov / 4.0)).ceil() as i32);
                    let mut previous = from;
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let next = unit(from * (1.0 - t) + to * t);
                        let p = projection.project(previous, 0.0);
                        let q = projection.project(next, 0.0);
                        if (p.visible || q.visible)
                            && dot(previous, projection.forward) > 0.0
                            && dot(next, projection.forward) > 0.0
                        {
                            result.points.push(StarInstance {
                                x: p.x as f32,
                                y: p.y as f32,
                                radius: q.x as f32,
                                kind: 4.0,
                                r: 0.26,
                                g: 0.52,
                                b: 0.68,
                                brightness: 0.18,
                                phase: q.y as f32,
                                limb: 0.0,
                                pad0: 0.0,
                                pad1: 0.0,
                            });
                        }
                        previous = next;
                    }
                }
            }
        }
    }
    let mut bodies = sky.bodies.clone();
    bodies.sort_by(|a, b| b.distance_au.total_cmp(&a.distance_au));
    for o in bodies {
        add_object(&mut result, &o, view, &projection);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn objects_cull_by_ground_magnitude_and_discs_sort_by_distance() {
        let scenario = Scenario {
            atmosphere: false,
            ground: false,
            magnitude: 7.0,
            ..Scenario::default()
        };
        let camera = scenario.camera(1280.0, 800.0);
        let ray = camera.forward();
        let star = Object {
            icrs: ray,
            magnitude: 1.0,
            illuminance_lux: photometry::illuminance(1.0),
            ..Object::default()
        };
        let faint = Object {
            magnitude: 8.0,
            ..star
        };
        let behind = Object {
            icrs: ray * -1.0,
            ..star
        };
        let sky = SkySnapshot {
            scenario: scenario.clone(),
            solar_visibility: 1.0,
            stars: vec![star, faint, behind],
            bodies: vec![
                Object {
                    body: 301,
                    observed: ray,
                    geometric: ray,
                    angular_radius: 0.005,
                    distance_au: 0.003,
                    unocculted_lux: 0.2,
                    illuminance_lux: 0.2,
                    ..Object::default()
                },
                Object {
                    body: 10,
                    observed: ray,
                    geometric: ray,
                    angular_radius: 0.005,
                    distance_au: 1.0,
                    unocculted_lux: photometry::SOLAR_LUX,
                    illuminance_lux: photometry::SOLAR_LUX,
                    ..Object::default()
                },
            ],
            ..SkySnapshot::default()
        };
        let scene = render_scene(&sky, &sky, &scenario, &camera, None);
        assert_eq!(scene.points.len(), 3);
        assert_eq!(
            scene
                .points
                .iter()
                .map(|p| p.kind as i32)
                .collect::<Vec<_>>(),
            vec![0, 1, 3]
        );
        assert_eq!(scene.points[0].radius, 3.0);
        assert!(
            scene
                .points
                .iter()
                .all(|p| p.brightness.is_finite() && p.brightness > 0.0)
        );
    }
    #[test]
    fn instance_layout_has_three_vec4_attributes() {
        assert_eq!(std::mem::size_of::<StarInstance>(), 48);
        assert_eq!(std::mem::offset_of!(StarInstance, r), 16);
        assert_eq!(std::mem::offset_of!(StarInstance, phase), 32);
    }
    #[test]
    fn cached_astrometry_uses_current_ground_horizon_pollution_and_magnitude() {
        let original = Scenario {
            atmosphere: false,
            ground: true,
            elevation: -5.0,
            light_pollution: 0.08,
            ..Scenario::default()
        };
        let camera = original.camera(1280.0, 800.0);
        let ray = camera.forward();
        let sky = SkySnapshot {
            scenario: original.clone(),
            stars: vec![Object {
                icrs: ray,
                magnitude: 1.0,
                illuminance_lux: photometry::illuminance(1.0),
                ..Object::default()
            }],
            bodies: vec![Object {
                body: 301,
                observed: ray,
                geometric: ray,
                distance_au: 0.003,
                angular_radius: 0.005,
                illuminance_lux: 0.2,
                unocculted_lux: 0.2,
                ..Object::default()
            }],
            ..SkySnapshot::default()
        };
        let mut view = original.clone();
        assert!(
            render_scene(&sky, &sky, &view, &camera, None)
                .points
                .is_empty()
        );
        view.ground = false;
        view.light_pollution = 0.6;
        let open = render_scene(&sky, &sky, &view, &camera, None);
        assert!(!open.ground);
        assert_eq!(open.pollution, 0.6_f32);
        assert_eq!(open.points.len(), 2);
        view.ground = true;
        assert!(
            render_scene(&sky, &sky, &view, &camera, None)
                .points
                .is_empty()
        );
        view.horizon.degrees.fill(-10.0);
        let low_horizon = render_scene(&sky, &sky, &view, &camera, None);
        assert_eq!(low_horizon.points.len(), 2);
        assert!(low_horizon.ground);
        assert_eq!(low_horizon.horizon.degrees[0], -10.0);
        view.magnitude = 0.0;
        let brighter_only = render_scene(&sky, &sky, &view, &camera, None);
        assert_eq!(brighter_only.points.len(), 1);
        assert_eq!(brighter_only.points[0].kind, 3.0);
        assert_eq!(sky.scenario, original);
    }
}
