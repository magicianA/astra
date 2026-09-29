use crate::camera::{DEFAULT_CAMERA_FOV, ProjectionKind};
use crate::{
    catalog::*,
    eclipse::{LunarSurface, disc_overlap, lunar_sunlight},
    ffi::*,
    math::*,
    orientation::Orientation,
    photometry,
    scenario::{Scenario, restore_day_night, validate},
    spice::*,
    time::*,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    path::Path,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub catalog_index: u32,
    pub hip: u32,
    pub flags: u32,
    pub body: i32,
    pub geometric: Vec3,
    pub observed: Vec3,
    pub icrs: Vec3,
    pub illumination: Vec3,
    pub magnitude: f64,
    pub distance_au: f64,
    pub angular_radius: f64,
    pub phase: f64,
    pub bright_limb_angle: f64,
    pub formal_error_arcsec: f64,
    pub color: [f32; 3],
    pub unocculted_lux: f64,
    pub illuminance_lux: f64,
}
impl Default for Object {
    fn default() -> Self {
        Self {
            id: 0,
            catalog_index: u32::MAX,
            hip: 0,
            flags: 0,
            body: 0,
            geometric: Vec3::default(),
            observed: Vec3::default(),
            icrs: Vec3::default(),
            illumination: Vec3::default(),
            magnitude: 0.0,
            distance_au: 0.0,
            angular_radius: 0.0,
            phase: 1.0,
            bright_limb_angle: 0.0,
            formal_error_arcsec: 0.0,
            color: [1.0; 3],
            unocculted_lux: 0.0,
            illuminance_lux: 0.0,
        }
    }
}
impl Object {
    pub fn azimuth(&self) -> f64 {
        wrap(self.observed.x.atan2(self.observed.y))
    }
    pub fn altitude(&self) -> f64 {
        self.observed.z.clamp(-1.0, 1.0).asin()
    }
}
#[derive(Clone, Debug)]
pub struct SkySnapshot {
    pub scenario: Scenario,
    pub time: TimeContext,
    pub generation: u64,
    pub celestial_to_enu: Mat3,
    pub stars: Vec<Object>,
    pub bodies: Vec<Object>,
    pub label_stars: Vec<usize>,
    pub guide_stars: HashMap<u32, Object>,
    pub data_id: String,
    pub compute_ms: f64,
    pub sun_altitude: f64,
    pub moon_altitude: f64,
    pub moon_phase: f64,
    pub catalog_count: usize,
    pub lunar: LunarSurface,
    pub solar_visibility: f64,
}
impl Default for SkySnapshot {
    fn default() -> Self {
        Self {
            scenario: Scenario::default(),
            time: TimeContext::default(),
            generation: 0,
            celestial_to_enu: Mat3::default(),
            stars: Vec::new(),
            bodies: Vec::new(),
            label_stars: Vec::new(),
            guide_stars: HashMap::new(),
            data_id: String::new(),
            compute_ms: 0.0,
            sun_altitude: -1.0,
            moon_altitude: -1.0,
            moon_phase: 0.0,
            catalog_count: 0,
            lunar: LunarSurface::default(),
            solar_visibility: 1.0,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    pub position: Vec3,
    pub velocity: Vec3,
}
static KERNEL_REFERENCES: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
fn kernel_pool() -> &'static Mutex<HashMap<String, usize>> {
    KERNEL_REFERENCES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn spice_check() -> Result<()> {
    // Every caller holds the process-global kernel pool lock.
    unsafe {
        if failed_c() != 0 {
            let mut message = [0i8; 2048];
            getmsg_c(c"LONG".as_ptr(), 2048, message.as_mut_ptr());
            reset_c();
            bail!(
                "SPICE: {}",
                CStr::from_ptr(message.as_ptr()).to_string_lossy()
            );
        }
    }
    Ok(())
}
fn release_kernel(pool: &mut HashMap<String, usize>, path: &str) {
    if let Some(count) = pool.get_mut(path) {
        *count -= 1;
        if *count == 0 {
            if let Ok(path_c) = CString::new(path) {
                unsafe {
                    unload_c(path_c.as_ptr());
                }
            }
            pool.remove(path);
        }
    }
}
pub struct Ephemeris {
    kernels: Vec<String>,
    lunar_start: f64,
    lunar_end: f64,
}
impl Ephemeris {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let mut pool = kernel_pool().lock().unwrap_or_else(|p| p.into_inner());
        // CSPICE mutates the supplied strings only for GET; SET consumes them.
        let mut action = b"RETURN\0".to_vec();
        let mut print = b"NONE\0".to_vec();
        unsafe {
            erract_c(c"SET".as_ptr(), 0, action.as_mut_ptr().cast());
            errprt_c(c"SET".as_ptr(), 0, print.as_mut_ptr().cast());
        }
        let mut result = Self {
            kernels: Vec::new(),
            lunar_start: 0.0,
            lunar_end: 0.0,
        };
        let setup = (|| -> Result<()> {
            for file in [
                "kernels/de441_part-1.bsp",
                "kernels/de441_part-2.bsp",
                "moon/pck00011.tpc",
                "moon/moon_de440_250416.tf",
                "moon/moon_pa_de440_200625.bpc",
            ] {
                let path = root.join(file);
                if !path.exists() {
                    bail!(
                        "Missing ephemeris kernel: {}. Run scripts/bootstrap.py --kernels",
                        path.display()
                    );
                }
                let path = path.canonicalize()?.to_string_lossy().into_owned();
                if !pool.contains_key(&path) {
                    let p = CString::new(path.as_str())?;
                    unsafe {
                        furnsh_c(p.as_ptr());
                    }
                    spice_check()?;
                }
                *pool.entry(path.clone()).or_default() += 1;
                result.kernels.push(path);
            }
            let mut storage = [0.0f64; 38];
            let mut coverage = SpiceCell {
                dtype: 1,
                length: 0,
                size: 32,
                card: 0,
                is_set: 1,
                adjust: 0,
                init: 0,
                base: storage.as_mut_ptr().cast(),
                data: unsafe { storage.as_mut_ptr().add(6).cast() },
            };
            let pck = CString::new(
                root.join("moon/moon_pa_de440_200625.bpc")
                    .to_string_lossy()
                    .as_bytes(),
            )?;
            unsafe {
                pckcov_c(pck.as_ptr(), 31008, &mut coverage);
            }
            spice_check()?;
            if unsafe { wncard_c(&coverage) } != 1 {
                bail!("Unexpected lunar orientation coverage");
            }
            unsafe {
                wnfetd_c(&coverage, 0, &mut result.lunar_start, &mut result.lunar_end);
            }
            spice_check()?;
            Ok(())
        })();
        if let Err(error) = setup {
            for p in result.kernels.drain(..) {
                release_kernel(&mut pool, &p);
            }
            return Err(error);
        }
        Ok(result)
    }
    pub fn state(&self, body: i32, jd: JulianDate) -> Result<State> {
        let _guard = kernel_pool().lock().unwrap_or_else(|p| p.into_inner());
        let mut pv = [0.0; 6];
        let mut lt = 0.0;
        let et = (jd.day - 2451545.0) * 86400.0 + jd.fraction * 86400.0;
        unsafe {
            spkez_c(
                body,
                et,
                c"J2000".as_ptr(),
                c"NONE".as_ptr(),
                0,
                pv.as_mut_ptr(),
                &mut lt,
            );
        }
        spice_check()?;
        Ok(State {
            position: Vec3 {
                x: pv[0],
                y: pv[1],
                z: pv[2],
            },
            velocity: Vec3 {
                x: pv[3],
                y: pv[4],
                z: pv[5],
            },
        })
    }
    pub fn moon_orientation(&self, tdb: JulianDate) -> Result<(Mat3, bool)> {
        let _guard = kernel_pool().lock().unwrap_or_else(|p| p.into_inner());
        let et = tdb.since_j2000() * 86400.0;
        let precise = et >= self.lunar_start && et <= self.lunar_end;
        let mut result = Mat3::default();
        unsafe {
            pxform_c(
                c"J2000".as_ptr(),
                if precise {
                    c"MOON_ME".as_ptr()
                } else {
                    c"IAU_MOON".as_ptr()
                },
                et,
                result.a.as_mut_ptr(),
            );
        }
        spice_check()?;
        Ok((result, precise))
    }
}
impl Drop for Ephemeris {
    fn drop(&mut self) {
        let mut pool = kernel_pool().lock().unwrap_or_else(|p| p.into_inner());
        for path in &self.kernels {
            release_kernel(&mut pool, path);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    FullSky,
    SolarSystem,
}
pub struct SkyEngine {
    eop: EarthOrientationData,
    orientation: Orientation,
    ephemeris: Ephemeris,
    pub catalog: Catalog,
}
impl SkyEngine {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        Ok(Self {
            eop: EarthOrientationData::new(root.join("time/finals2000A.all"))?,
            orientation: Orientation::new(root.join("time/cio.bin"))?,
            ephemeris: Ephemeris::new(root)?,
            catalog: Catalog::new(root)?,
        })
    }
    pub fn eop(&self) -> &EarthOrientationData {
        &self.eop
    }
    pub fn compute(
        &self,
        s: &Scenario,
        generation: u64,
        latest: Option<&AtomicU64>,
        scope: Scope,
        only_star: Option<u32>,
    ) -> Result<SkySnapshot> {
        let start = Instant::now();
        validate(s)?;
        let t = make_time_civil(
            &s.date,
            s.scale,
            s.longitude,
            &self.eop,
            s.override_delta_t,
            s.custom_delta_t,
            s.julian,
        )?;
        let c2t = self.orientation.terrestrial(&t)?;
        let enu = enu_basis(s.longitude * RAD, s.latitude * RAD);
        let mut out = SkySnapshot {
            scenario: s.clone(),
            time: t.clone(),
            generation,
            data_id: self.catalog.data_id.clone(),
            catalog_count: self.catalog.stars.len(),
            celestial_to_enu: enu * c2t,
            sun_altitude: -1.0,
            moon_altitude: -1.0,
            solar_visibility: 1.0,
            ..SkySnapshot::default()
        };
        let mut station = [0.0; 3];
        if unsafe {
            eraGd2gc(
                1,
                s.longitude * RAD,
                s.latitude * RAD,
                s.height,
                station.as_mut_ptr(),
            )
        } != 0
        {
            bail!("Invalid WGS84 station");
        }
        let site = (c2t.transpose() * vec(station)) / 1000.0;
        let mut before = t.clone();
        let mut after = t.clone();
        before.ut1 = t.ut1.add_seconds(-0.5);
        before.tt = t.tt.add_seconds(-0.5);
        after.ut1 = t.ut1.add_seconds(0.5);
        after.tt = t.tt.add_seconds(0.5);
        let site_velocity = (self.orientation.terrestrial(&after)?.transpose() * vec(station)
            - self.orientation.terrestrial(&before)?.transpose() * vec(station))
            / 1000.0;
        let earth = self.ephemeris.state(399, t.tdb)?;
        let sun = self.ephemeris.state(10, t.tdb)?;
        let observer = earth.position + site;
        let mut pv = [array(site * 1000.0), array(site_velocity * 1000.0)];
        let mut ebpv = [
            array(earth.position / AU_KM),
            array(earth.velocity * (86400.0 / AU_KM)),
        ];
        let mut ehp = array((earth.position - sun.position) / AU_KM);
        let mut astrom = EraAstrom::default();
        unsafe {
            eraApcs(
                t.tdb.day,
                t.tdb.fraction,
                pv.as_mut_ptr(),
                ebpv.as_mut_ptr(),
                ehp.as_mut_ptr(),
                &mut astrom,
            );
        }
        astrom.pmt = 0.0;
        let definitions: [(i32, f64, f64, [f32; 3]); 9] = [
            (10, 695700.0, -26.74, [1.0, 0.92, 0.65]),
            (301, 1737.4, -12.73, [0.8, 0.86, 0.93]),
            (199, 2439.7, -0.42, [0.75, 0.7, 0.62]),
            (299, 6051.8, -4.4, [1.0, 0.89, 0.69]),
            (4, 3389.5, -1.52, [1.0, 0.48, 0.25]),
            (5, 69911.0, -9.4, [1.0, 0.85, 0.65]),
            (6, 58232.0, -8.88, [0.91, 0.78, 0.52]),
            (7, 25362.0, -7.19, [0.46, 0.87, 0.89]),
            (8, 24622.0, -6.87, [0.35, 0.52, 1.0]),
        ];
        for (id, radius, m0, color) in definitions {
            let mut target = self.ephemeris.state(id, t.tdb)?;
            let mut light = 0.0;
            for _ in 0..8 {
                let next = norm(target.position - observer) / C_KMS;
                target = self.ephemeris.state(id, t.tdb.add_seconds(-next))?;
                if (next - light).abs() < 1e-8 {
                    light = next;
                    break;
                }
                light = next;
            }
            let r = target.position - observer;
            let distance = norm(r);
            let direction = unit(r);
            let mut p = array(direction);
            let mut apparent = [0.0; 3];
            if id != 10 {
                let mut q = array(unit(target.position - sun.position));
                let mut e = array(unit(observer - sun.position));
                let mut deflected = [0.0; 3];
                unsafe {
                    eraLd(
                        1.0,
                        p.as_mut_ptr(),
                        q.as_mut_ptr(),
                        e.as_mut_ptr(),
                        norm(observer - sun.position) / AU_KM,
                        1e-6,
                        deflected.as_mut_ptr(),
                    );
                }
                p = deflected;
            }
            unsafe {
                eraAb(
                    p.as_mut_ptr(),
                    astrom.v.as_mut_ptr(),
                    astrom.em,
                    astrom.bm1,
                    apparent.as_mut_ptr(),
                );
            }
            let mut o = Object {
                id: id as u64,
                body: id,
                icrs: vec(apparent),
                distance_au: distance / AU_KM,
                angular_radius: (radius / distance).clamp(0.0, 1.0).asin(),
                color,
                ..Object::default()
            };
            o.geometric = unit(out.celestial_to_enu * o.icrs);
            o.observed = if s.atmosphere {
                refract(o.geometric, s.pressure, s.temperature)
            } else {
                o.geometric
            };
            let illumination = unit(sun.position - target.position);
            o.illumination = out.celestial_to_enu * illumination;
            let ca = dot(illumination, unit(observer - target.position)).clamp(-1.0, 1.0);
            let phaseangle = ca.acos();
            o.phase = if id == 10 { 1.0 } else { (1.0 + ca) / 2.0 };
            o.magnitude = if id == 10 {
                -26.74 + 5.0 * o.distance_au.log10()
            } else if id == 301 {
                -12.73
                    + 1.49 * phaseangle
                    + 0.0431 * phaseangle.powi(4)
                    + 5.0 * (distance / 384400.0).log10()
                    + 5.0 * (norm(target.position - sun.position) / AU_KM).log10()
            } else {
                m0 + 5.0 * (o.distance_au * norm(target.position - sun.position) / AU_KM).log10()
                    - 2.5
                        * ((phaseangle.sin() + (PI - phaseangle) * ca) / PI)
                            .max(0.001)
                            .log10()
            };
            o.illuminance_lux = if id == 10 {
                photometry::SOLAR_LUX / (o.distance_au * o.distance_au)
            } else {
                photometry::illuminance(o.magnitude)
            };
            if id == 10 || id == 301 {
                for (i, c) in o.color.iter_mut().enumerate() {
                    *c = (photometry::SOLAR_RGB[i] / photometry::SOLAR_LUX) as f32;
                }
            }
            o.unocculted_lux = o.illuminance_lux;
            if id == 301 {
                let (fixed, precise) =
                    self.ephemeris.moon_orientation(t.tdb.add_seconds(-light))?;
                out.lunar.precise_orientation = precise;
                out.lunar.fixed_from_enu = fixed * out.celestial_to_enu.transpose();
                let surface_time = t.tdb.add_seconds(-light);
                let shadow_earth = self.ephemeris.state(
                    399,
                    surface_time.add_seconds(-norm(earth.position - target.position) / C_KMS),
                )?;
                let lighting_sun = self.ephemeris.state(
                    10,
                    surface_time.add_seconds(-norm(sun.position - target.position) / C_KMS),
                )?;
                out.lunar.sun_km = fixed * (lighting_sun.position - target.position);
                out.lunar.earth_km = fixed * (shadow_earth.position - target.position);
                let facing = unit(out.lunar.earth_km);
                let axis = unit(cross(
                    facing,
                    Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                ));
                let up = cross(axis, facing);
                let mut visibility = 0.0;
                for i in 0..128 {
                    let r = ((i as f64 + 0.5) / 128.0).sqrt();
                    let phi = i as f64 * 2.399963229728653;
                    let surface = (axis * (r * phi.cos())
                        + up * (r * phi.sin())
                        + facing * (1.0 - r * r).sqrt())
                        * 1737.4;
                    visibility +=
                        lunar_sunlight(out.lunar.sun_km - surface, out.lunar.earth_km - surface);
                }
                out.lunar.solar_visibility = visibility / 128.0;
                o.illuminance_lux *=
                    out.lunar.solar_visibility + 0.001 * (1.0 - out.lunar.solar_visibility);
                let earth_phase =
                    (1.0 + dot(
                        unit(target.position - earth.position),
                        unit(sun.position - earth.position),
                    )) / 2.0;
                out.lunar.earthshine_lux = if s.earthshine {
                    photometry::SOLAR_LUX
                        * 0.3
                        * (6378.137 / norm(out.lunar.earth_km)).powi(2)
                        * photometry::lambert_phase(earth_phase)
                } else {
                    0.0
                };
            }
            let east = unit(cross(
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                o.geometric,
            ));
            let north = cross(o.geometric, east);
            o.bright_limb_angle = dot(o.illumination, north).atan2(dot(o.illumination, east));
            if id == 10 {
                out.sun_altitude = o.altitude();
            }
            if id == 301 {
                out.moon_altitude = o.altitude();
                out.moon_phase = o.phase;
            }
            out.bodies.push(o);
        }
        let solar = out.bodies[0];
        let moon = out.bodies[1];
        out.solar_visibility = 1.0
            - disc_overlap(
                solar.angular_radius,
                moon.angular_radius,
                angle(solar.observed, moon.observed),
            );
        out.bodies[0].illuminance_lux *= out.solar_visibility;
        if scope == Scope::SolarSystem {
            out.compute_ms = start.elapsed().as_secs_f64() * 1000.0;
            return Ok(out);
        }
        let mut tcb1 = 0.0;
        let mut tcb2 = 0.0;
        unsafe {
            eraTdbtcb(t.tdb.day, t.tdb.fraction, &mut tcb1, &mut tcb2);
        }
        if only_star.is_some_and(|i| i as usize >= self.catalog.stars.len()) {
            bail!("Invalid catalogue index");
        }
        out.stars.reserve(30000);
        let first = only_star.unwrap_or(0) as usize;
        let end = if only_star.is_some() {
            first + 1
        } else {
            self.catalog.stars.len()
        };
        for i in first..end {
            if i & 4095 == 0 && cancelled(generation, latest) {
                bail!("Computation cancelled");
            }
            let star = self.catalog.stars[i];
            let guide = s.constellations && self.catalog.constellation_member[i];
            let days = if star.flags & GAIA != 0 {
                (tcb1 - 2451545.0) + tcb2
            } else {
                t.tdb.since_j2000()
            };
            let years = 2000.0 + days / 365.25 - star.epoch;
            let plx = if star.flags & DISTANCE_UNKNOWN != 0 {
                0.0
            } else {
                star.parallax
            };
            let mut mag = star.magnitude as f64;
            let lower = if plx > 0.0 {
                (1.0 + star.rv * years * 31557600.0 * plx * ARCSEC / AU_KM).abs()
            } else {
                1.0
            };
            if only_star.is_none()
                && !guide
                && mag + 5.0 * lower.max(1e-12).log10() > s.magnitude + 0.4
            {
                continue;
            }
            let (mut ra2, mut de2, mut pmr, mut pmd, mut px2, mut rv2) =
                (star.ra, star.dec, 0.0, 0.0, plx, star.rv);
            let mut quality = star.flags;
            if plx > 0.0 {
                let epoch = 2451545.0 + (star.epoch - 2000.0) * 365.25;
                let status = unsafe {
                    eraPmsafe(
                        star.ra,
                        star.dec,
                        star.pmra / star.dec.cos().max(1e-12),
                        star.pmdec,
                        plx,
                        star.rv,
                        2451545.0,
                        epoch - 2451545.0,
                        2451545.0,
                        days,
                        &mut ra2,
                        &mut de2,
                        &mut pmr,
                        &mut pmd,
                        &mut px2,
                        &mut rv2,
                    )
                };
                if status < 0 {
                    continue;
                }
                if status > 0 {
                    quality |= PROPAGATION_WARNING;
                }
                if px2 > 0.0 {
                    mag -= 5.0 * (px2 / plx).log10();
                }
            } else {
                let u = sphere(star.ra, star.dec);
                let ea = Vec3 {
                    x: -star.ra.sin(),
                    y: star.ra.cos(),
                    z: 0.0,
                };
                let ed = Vec3 {
                    x: -star.dec.sin() * star.ra.cos(),
                    y: -star.dec.sin() * star.ra.sin(),
                    z: star.dec.cos(),
                };
                let u = unit(u + (ea * star.pmra + ed * star.pmdec) * years);
                ra2 = u.y.atan2(u.x);
                de2 = u.z.asin();
            }
            if only_star.is_none() && !guide && mag > s.magnitude + 0.4 {
                continue;
            }
            let mut rai = 0.0;
            let mut dei = 0.0;
            unsafe {
                eraAtciq(
                    ra2,
                    de2,
                    0.0,
                    0.0,
                    px2,
                    0.0,
                    &mut astrom,
                    &mut rai,
                    &mut dei,
                );
            }
            let mut o = Object {
                id: star.id,
                hip: star.hip,
                catalog_index: i as u32,
                flags: quality,
                icrs: sphere(rai, dei),
                magnitude: mag,
                distance_au: if px2 > 0.0 { 1.0 / (px2 * ARCSEC) } else { 0.0 },
                color: star_color(star.bv as f64),
                ..Object::default()
            };
            o.geometric = unit(out.celestial_to_enu * o.icrs);
            o.observed = if s.atmosphere {
                refract(o.geometric, s.pressure, s.temperature)
            } else {
                o.geometric
            };
            let visual = if star.flags & GAIA != 0 {
                photometry::gaia_visual_magnitude(mag, star.bv as f64)
            } else {
                mag
            };
            o.illuminance_lux = photometry::illuminance(visual);
            for channel in &mut o.color {
                *channel = if *channel <= 0.04045 {
                    *channel / 12.92
                } else {
                    ((*channel + 0.055) / 1.055).powf(2.4)
                };
            }
            o.formal_error_arcsec = (star.ra_error as f64)
                .hypot(star.dec_error as f64)
                .hypot(years.abs() * (star.pmra_error as f64).hypot(star.pmdec_error as f64))
                / 1000.0;
            if o.magnitude <= 2.7 && !(o.flags & GAIA != 0 && o.hip == 0) {
                out.label_stars.push(out.stars.len());
            }
            if guide {
                out.guide_stars.insert(i as u32, o);
            }
            out.stars.push(o);
        }
        out.compute_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(out)
    }
}
fn vec(a: [f64; 3]) -> Vec3 {
    Vec3 {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}
fn array(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}
fn cancelled(generation: u64, latest: Option<&AtomicU64>) -> bool {
    latest.is_some_and(|v| v.load(Ordering::SeqCst) != generation)
}
pub fn refraction(alt: f64, pressure: f64, temperature: f64) -> f64 {
    if pressure <= 0.0 || alt < -2.0 * RAD || alt > 89.9 * RAD {
        return 0.0;
    }
    let d = (alt / RAD).max(-1.0);
    let mut r = 1.02 / ((d + 10.3 / (d + 5.11)) * RAD).tan() / 60.0
        * RAD
        * (pressure / 1010.0)
        * (283.0 / (273.0 + temperature));
    if alt < -RAD {
        r *= (alt / RAD + 2.0).clamp(0.0, 1.0);
    }
    r.max(0.0)
}
pub fn refract(v: Vec3, pressure: f64, temp: f64) -> Vec3 {
    let altitude = v.z.clamp(-1.0, 1.0).asin();
    let correction = refraction(altitude, pressure, temp);
    if correction == 0.0 {
        return v;
    }
    let horizontal = v.x.hypot(v.y);
    if altitude + correction >= PI / 2.0 || horizontal < 1e-15 {
        return Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
    }
    let (sine, cosine) = correction.sin_cos();
    let ratio = cosine - v.z * sine / horizontal;
    Vec3 {
        x: v.x * ratio,
        y: v.y * ratio,
        z: v.z * cosine + horizontal * sine,
    }
}
pub fn body_name(body: i32) -> &'static str {
    match body {
        10 => "太阳 Sun",
        301 => "月球 Moon",
        199 => "水星 Mercury",
        299 => "金星 Venus",
        4 => "火星 Mars",
        5 => "木星 Jupiter",
        6 => "土星 Saturn",
        7 => "天王星 Uranus",
        8 => "海王星 Neptune",
        _ => "?",
    }
}
pub fn quality_text(o: &Object) -> String {
    if o.body != 0 {
        return if (4..=8).contains(&o.body) {
            "DE441 · 行星系统质心近似"
        } else {
            "DE441 · 站心视位置"
        }
        .to_owned();
    }
    let mut s = if o.flags & GAIA != 0 {
        "Gaia DR3"
    } else {
        "Hipparcos-2"
    }
    .to_owned();
    for (flag, text) in [
        (RV_UNKNOWN, " · 径向速度未知"),
        (DISTANCE_UNKNOWN, " · 距离未知"),
        (PM_UNKNOWN, " · 固定方向近似"),
        (MULTIPLE, " · 多星/光心近似"),
        (LOW_QUALITY, " · 低质量解"),
        (PROPAGATION_WARNING, " · 运动传播近似警告"),
        (POSITION_MATCH, " · HIP 位置匹配"),
    ] {
        if o.flags & flag != 0 {
            s.push_str(text);
        }
    }
    s
}
pub fn observe_star(star: &Object, frame: &SkySnapshot) -> Object {
    let mut result = *star;
    result.geometric = frame.celestial_to_enu * star.icrs;
    let s = &frame.scenario;
    result.observed = if s.atmosphere {
        refract(result.geometric, s.pressure, s.temperature)
    } else {
        result.geometric
    };
    result
}

impl SkyEngine {
    pub fn next_moon_view(
        &self,
        initial: &Scenario,
        generation: u64,
        latest: Option<&AtomicU64>,
    ) -> Result<Option<Scenario>> {
        validate(initial)?;
        let start = to_jd(&initial.date, initial.julian)?;
        for step in 0..=35 * 48 {
            if cancelled(generation, latest) {
                return Ok(None);
            }
            let mut candidate = initial.clone();
            candidate.date = from_jd(start.add_seconds(step as f64 * 1800.0), initial.julian);
            if candidate.date.year >= 7000
                || (candidate.scale == TimeScale::UTC && candidate.date.year >= 2027)
            {
                break;
            }
            let sky = self.compute(&candidate, generation, latest, Scope::SolarSystem, None)?;
            if sky.moon_altitude >= 8.0 * RAD
                && sky.sun_altitude <= -6.0 * RAD
                && sky.moon_phase >= 0.02
                && (!initial.ground || initial.horizon.visible(sky.bodies[1].observed, 0.0))
            {
                let moon = sky.bodies[1];
                candidate.azimuth = moon.azimuth() / RAD;
                candidate.elevation = moon.altitude() / RAD;
                candidate.roll = 0.0;
                candidate.fov = 6.0;
                candidate.projection = ProjectionKind::Perspective;
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
    pub fn twilight_view(
        &self,
        initial: &Scenario,
        dawn: bool,
        direction: i32,
        generation: u64,
        latest: Option<&AtomicU64>,
    ) -> Result<Option<Scenario>> {
        validate(initial)?;
        if direction != -1 && direction != 1 {
            bail!("Twilight search direction must be -1 or +1");
        }
        let start = to_jd(&initial.date, initial.julian)?;
        let sample = |seconds: f64| -> Result<Option<Object>> {
            if cancelled(generation, latest) {
                return Ok(None);
            }
            let mut candidate = initial.clone();
            candidate.date = from_jd(start.add_seconds(seconds), initial.julian);
            if candidate.date.year < -3000
                || candidate.date.year >= 7000
                || (candidate.scale == TimeScale::UTC
                    && (candidate.date.year < 1973 || candidate.date.year >= 2027))
            {
                return Ok(None);
            }
            Ok(self
                .compute(&candidate, generation, latest, Scope::SolarSystem, None)?
                .bodies
                .into_iter()
                .find(|b| b.body == 10))
        };
        let threshold = -0.10452846326765347;
        let mut previous_time = direction as f64 * 2.0;
        let Some(mut previous) = sample(previous_time)? else {
            return Ok(None);
        };
        for step in 1..=370 * 48 {
            let current_time = direction as f64 * (2.0 + step as f64 * 1800.0);
            let Some(current) = sample(current_time)? else {
                return Ok(None);
            };
            let left_value = if direction > 0 {
                previous.geometric.z
            } else {
                current.geometric.z
            } - threshold;
            let right_value = if direction > 0 {
                current.geometric.z
            } else {
                previous.geometric.z
            } - threshold;
            let mut crossing = if dawn {
                left_value < 0.0 && right_value >= 0.0
            } else {
                left_value > 0.0 && right_value <= 0.0
            };
            let mut left = previous_time.min(current_time);
            let mut right = previous_time.max(current_time);
            if !crossing
                && left_value * right_value > 0.0
                && left_value.abs().min(right_value.abs()) < 0.005
            {
                let maximize = left_value < 0.0;
                let mut lo = left;
                let mut hi = right;
                for _ in 0..24 {
                    let t1 = lo + (hi - lo) / 3.0;
                    let t2 = hi - (hi - lo) / 3.0;
                    let Some(sun1) = sample(t1)? else {
                        return Ok(None);
                    };
                    let Some(sun2) = sample(t2)? else {
                        return Ok(None);
                    };
                    if (sun1.geometric.z < sun2.geometric.z) == maximize {
                        lo = t1;
                    } else {
                        hi = t2;
                    }
                }
                let middle = (lo + hi) / 2.0;
                let Some(sun) = sample(middle)? else {
                    return Ok(None);
                };
                let value = sun.geometric.z - threshold;
                if if maximize { value > 0.0 } else { value < 0.0 } {
                    crossing = true;
                    if dawn == maximize {
                        right = middle;
                    } else {
                        left = middle;
                    }
                }
            }
            if crossing {
                while right - left > 0.1 {
                    let middle = (left + right) / 2.0;
                    let Some(sun) = sample(middle)? else {
                        return Ok(None);
                    };
                    if (sun.geometric.z < threshold) == dawn {
                        left = middle;
                    } else {
                        right = middle;
                    }
                }
                let seconds = (left + right) / 2.0;
                let Some(sun) = sample(seconds)? else {
                    return Ok(None);
                };
                let mut candidate = initial.clone();
                candidate.date = from_jd(start.add_seconds(seconds), initial.julian);
                candidate.azimuth = sun.azimuth() / RAD;
                candidate.elevation = 8.0;
                candidate.roll = 0.0;
                candidate.fov = DEFAULT_CAMERA_FOV / RAD;
                candidate.projection = ProjectionKind::Perspective;
                restore_day_night(&mut candidate);
                candidate.ground = true;
                return Ok(Some(candidate));
            }
            previous = current;
            previous_time = current_time;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventKind, EventSearch, Target, find_events, observing_plan, scene_at};
    #[test]
    fn refraction_matches_vertical_plane_reference() {
        assert!(refraction(0.0, 1013.0, 15.0) > 0.0);
        assert_eq!(refraction(45.0 * RAD, 0.0, 15.0), 0.0);
        for temperature in [-90.0, 15.0, 60.0] {
            for degrees in (-90..=90).step_by(3) {
                for azimuth in [0.0f64, 1.1, 2.7, 5.2] {
                    let altitude = degrees as f64 * RAD;
                    let original = Vec3 {
                        x: azimuth.sin() * altitude.cos(),
                        y: azimuth.cos() * altitude.cos(),
                        z: altitude.sin(),
                    };
                    let corrected =
                        (altitude + refraction(altitude, 1200.0, temperature)).min(PI / 2.0);
                    let reference = Vec3 {
                        x: azimuth.sin() * corrected.cos(),
                        y: azimuth.cos() * corrected.cos(),
                        z: corrected.sin(),
                    };
                    let rotated = refract(original, 1200.0, temperature);
                    assert!(angle(rotated, reference) < 1e-12);
                    assert!((norm(rotated) - 1.0).abs() < 1e-12);
                }
            }
        }
    }
    #[test]
    fn astronomical_data_and_independent_event_references() -> Result<()> {
        let data = crate::source_data();
        let engine = SkyEngine::new(&data)?;
        assert_eq!(engine.catalog.constellations.len(), 88);
        assert!(
            engine
                .catalog
                .constellations
                .iter()
                .all(|c| !c.segments.is_empty())
        );
        let sirius = engine.catalog.search("  hip 32349 ", 6);
        assert_eq!(sirius.len(), 1);
        assert_eq!(engine.catalog.stars[sirius[0] as usize].hip, 32349);
        assert_eq!(engine.catalog.search("sIrIuS", 6), sirius);
        let gaia = engine
            .catalog
            .stars
            .iter()
            .find(|s| s.flags & GAIA != 0)
            .unwrap();
        let found = engine.catalog.search(format!("Gaia DR3 {}", gaia.id), 6);
        assert_eq!(found.len(), 1);
        assert_eq!(engine.catalog.stars[found[0] as usize].id, gaia.id);
        let start = Instant::now();
        for _ in 0..100 {
            assert!(engine.catalog.search("no-such-object-973", 6).is_empty());
        }
        assert!(start.elapsed().as_secs_f64() / 100.0 < 0.02);
        let mut scene = Scenario {
            magnitude: -2.0,
            ..Scenario::default()
        };
        assert_eq!(
            engine
                .compute(&scene, 0, None, Scope::FullSky, Some(sirius[0]))?
                .stars
                .len(),
            1
        );
        let beijing = Scenario::default();
        let before_moonrise = engine.compute(&beijing, 0, None, Scope::SolarSystem, None)?;
        assert!(
            before_moonrise.stars.is_empty()
                && before_moonrise.bodies.len() == 9
                && before_moonrise.moon_altitude < 0.0
        );
        let moon_view = engine
            .next_moon_view(&beijing, 0, None)?
            .expect("Moon becomes visible tonight");
        assert_eq!(moon_view.longitude, beijing.longitude);
        assert_eq!(moon_view.latitude, beijing.latitude);
        assert_eq!(moon_view.scale, beijing.scale);
        assert_eq!(moon_view.julian, beijing.julian);
        let elapsed = to_jd(&moon_view.date, false)?.value() - to_jd(&beijing.date, false)?.value();
        assert!(elapsed > 0.0 && elapsed < 1.0);
        let moon_sky = engine.compute(&moon_view, 0, None, Scope::SolarSystem, None)?;
        assert!(moon_sky.moon_altitude >= 8.0 * RAD && moon_sky.sun_altitude <= -6.0 * RAD);
        assert!((moon_view.elevation * RAD - moon_sky.moon_altitude).abs() < 1e-10);
        let cancelled = AtomicU64::new(2);
        assert!(
            engine
                .next_moon_view(&beijing, 1, Some(&cancelled))?
                .is_none()
        );
        let polar_day = Scenario {
            latitude: 90.0,
            date: CivilDate {
                year: 2026,
                month: 6,
                day: 1,
                hour: 12,
                minute: 0,
                second: 0.0,
            },
            ..beijing.clone()
        };
        assert!(engine.next_moon_view(&polar_day, 0, None)?.is_none());
        let star_epoch = engine.compute(&beijing, 0, None, Scope::FullSky, None)?;
        let playback_end = scene_at(&beijing, 540.0)?;
        let exact_end = engine.compute(&playback_end, 0, None, Scope::FullSky, None)?;
        let exact: HashMap<_, _> = exact_end.stars.iter().map(|o| (o.id, o)).collect();
        for star in &star_epoch.stars {
            if star.magnitude > 3.0 || star.altitude() < 10.0 * RAD {
                continue;
            }
            let displayed = observe_star(star, &exact_end);
            assert!(angle(displayed.observed, exact[&star.id].observed) / ARCSEC < 0.2);
        }
        for direction in [-1, 1] {
            let reference = Object {
                icrs: Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                ..Object::default()
            };
            let mut previous_star = Vec3::default();
            let mut previous_moon = Vec3::default();
            let mut minimum_step = 1.0f64;
            let mut maximum_step = 0.0f64;
            for frame in 0..=60 {
                let mut clock = scene_at(&beijing, (direction * frame * 9) as f64)?;
                clock.atmosphere = false;
                let instant = engine.compute(&clock, 0, None, Scope::SolarSystem, None)?;
                let displayed = observe_star(&reference, &instant);
                let moon = instant.bodies[1];
                if frame > 0 {
                    let step = angle(previous_star, displayed.observed);
                    minimum_step = minimum_step.min(step);
                    maximum_step = maximum_step.max(step);
                    assert!(step > 1e-5);
                    assert!(angle(previous_moon, moon.observed) > 1e-6);
                }
                previous_star = displayed.observed;
                previous_moon = moon.observed;
            }
            assert!(maximum_step / minimum_step < 1.001);
        }
        scene.magnitude = 6.5;
        scene.atmosphere = false;
        scene.scale = TimeScale::UT1;
        for year in [-3000, 0, 1969, 2000, 2026, 4000, 6999] {
            scene.date = CivilDate {
                year,
                month: 1,
                day: 2,
                hour: 12,
                minute: 0,
                second: 0.0,
            };
            let sky = engine.compute(&scene, 0, None, Scope::FullSky, None)?;
            assert_eq!(sky.bodies.len(), 9);
            assert!(sky.stars.len() > 3000);
            for o in sky.bodies {
                assert!(o.altitude().is_finite() && o.magnitude.is_finite() && o.distance_au > 0.0);
                assert!((norm(o.observed) - 1.0).abs() < 1e-12);
            }
        }
        scene.date = CivilDate {
            year: 2026,
            month: 9,
            day: 4,
            hour: 22,
            minute: 0,
            second: 0.0,
        };
        for latitude in [-90.0, 90.0] {
            scene.latitude = latitude;
            assert!(
                engine
                    .compute(&scene, 0, None, Scope::SolarSystem, None)?
                    .bodies[0]
                    .azimuth()
                    .is_finite()
            );
        }
        {
            let temporary = Ephemeris::new(&data)?;
            let sky = engine.compute(&scene, 0, None, Scope::SolarSystem, None)?;
            assert!(norm(temporary.state(399, sky.time.tdb)?.position) > 1e7);
        }
        assert_eq!(
            engine
                .compute(&scene, 0, None, Scope::SolarSystem, None)?
                .bodies
                .len(),
            9
        );
        scene = Scenario {
            date: CivilDate {
                year: 2026,
                month: 9,
                day: 4,
                hour: 12,
                minute: 0,
                second: 0.0,
            },
            scale: TimeScale::UT1,
            atmosphere: false,
            auto_exposure: false,
            exposure: 3.0,
            ..Scenario::default()
        };
        let solar_altitude = |s: &Scenario| -> Result<f64> {
            Ok(
                engine.compute(s, 0, None, Scope::SolarSystem, None)?.bodies[0]
                    .geometric
                    .z
                    .asin()
                    / RAD,
            )
        };
        for dawn in [false, true] {
            scene.adaptive_exposure = dawn;
            for direction in [-1, 1] {
                let event = engine
                    .twilight_view(&scene, dawn, direction, 0, None)?
                    .expect("civil twilight");
                assert!(event.atmosphere && event.auto_exposure && event.exposure == 1.0);
                assert_eq!(event.adaptive_exposure, scene.adaptive_exposure);
                assert!((solar_altitude(&event)? + 6.0).abs() < 0.0003);
                let seconds = (to_jd(&event.date, false)?.value()
                    - to_jd(&scene.date, false)?.value())
                    * 86400.0;
                assert!(seconds * direction as f64 > 0.0 && seconds.abs() < 86400.0);
                let mut after = event.clone();
                after.date = from_jd(to_jd(&after.date, false)?.add_seconds(60.0), false);
                assert_eq!(solar_altitude(&after)? > -6.0, dawn);
                let geometric = solar_altitude(&after)?;
                after.atmosphere = false;
                assert!((solar_altitude(&after)? - geometric).abs() < 1e-10);
                let again = engine
                    .twilight_view(&event, dawn, direction, 0, None)?
                    .expect("next twilight");
                assert!(
                    (to_jd(&again.date, false)?.value() - to_jd(&event.date, false)?.value()).abs()
                        > 0.9
                );
            }
        }
        let cancelled = AtomicU64::new(2);
        assert!(
            engine
                .twilight_view(&scene, true, 1, 1, Some(&cancelled))?
                .is_none()
        );
        let mut boundary = scene.clone();
        boundary.date = CivilDate {
            year: 6999,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59.0,
        };
        assert!(engine.twilight_view(&boundary, true, 1, 0, None)?.is_none());
        boundary.date = CivilDate {
            year: -3000,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0.0,
        };
        assert!(
            engine
                .twilight_view(&boundary, true, -1, 0, None)?
                .is_none()
        );
        let mut polar = scene.clone();
        polar.latitude = 90.0;
        polar.date = CivilDate {
            year: 2026,
            month: 6,
            day: 21,
            hour: 12,
            minute: 0,
            second: 0.0,
        };
        let dusk = engine
            .twilight_view(&polar, false, 1, 0, None)?
            .expect("seasonal polar dusk");
        assert!(dusk.date.month >= 9);
        assert!((solar_altitude(&dusk)? + 6.0).abs() < 0.0003);
        let mut grazing = scene.clone();
        grazing.latitude = 72.55;
        grazing.longitude = 0.0;
        grazing.scale = TimeScale::LocalMean;
        grazing.date = CivilDate {
            year: 2026,
            month: 12,
            day: 21,
            hour: 10,
            minute: 15,
            second: 0.0,
        };
        let dawn = engine
            .twilight_view(&grazing, true, 1, 0, None)?
            .expect("grazing dawn");
        let dusk = engine
            .twilight_view(&grazing, false, 1, 0, None)?
            .expect("grazing dusk");
        assert_eq!(dawn.date.day, 21);
        assert_eq!(dusk.date.day, 21);
        let duration =
            (to_jd(&dusk.date, false)?.value() - to_jd(&dawn.date, false)?.value()) * 86400.0;
        assert!(duration > 0.0 && duration < 1800.0);
        assert!((solar_altitude(&dawn)? + 6.0).abs() < 0.0003);
        assert!((solar_altitude(&dusk)? + 6.0).abs() < 0.0003);
        scene = Scenario {
            date: CivilDate {
                year: 2024,
                month: 4,
                day: 8,
                hour: 0,
                minute: 0,
                second: 0.0,
            },
            scale: TimeScale::UTC,
            latitude: 32.0 + 17.0 / 60.0,
            longitude: -(96.0 + 42.0 / 60.0),
            height: 0.0,
            ..Scenario::default()
        };
        let mut query = EventSearch {
            days: 1.0,
            ..EventSearch::default()
        };
        let found = find_events(&engine, &scene, &query, None)?;
        assert_eq!(found.len(), 1);
        let event = &found[0];
        assert_eq!(event.r#type, "Total solar eclipse");
        assert!(event.visible);
        assert!((event.peak - (18.0 * 3600.0 + 42.0 * 60.0)).abs() < 60.0);
        assert!(
            event.start.unwrap() < event.inner_start.unwrap()
                && event.inner_start.unwrap() < event.peak
                && event.peak < event.inner_end.unwrap()
                && event.inner_end.unwrap() < event.end.unwrap()
        );
        let peak = engine.compute(
            &scene_at(&scene, event.peak)?,
            0,
            None,
            Scope::SolarSystem,
            None,
        )?;
        assert!(peak.solar_visibility < 0.001 && peak.lunar.precise_orientation);
        query.days = 1200.0 / 86400.0;
        let short = find_events(
            &engine,
            &scene_at(&scene, event.peak - 500.0)?,
            &query,
            None,
        )?;
        assert_eq!(short.len(), 1);
        assert!((short[0].peak - 500.0).abs() < 1.0);
        query.days = 1.0;
        for offset in [600.0, 86000.0] {
            let edge = find_events(
                &engine,
                &scene_at(&scene, event.peak - offset)?,
                &query,
                None,
            )?;
            assert_eq!(edge.len(), 1);
            assert!((edge[0].peak - offset).abs() < 1.0);
        }
        query.kind = EventKind::Occultation;
        query.first = Target::default();
        query.second = Target {
            body: 10,
            star: None,
        };
        let occultation = find_events(&engine, &scene, &query, None)?;
        assert_eq!(occultation.len(), 1);
        assert!(occultation[0].obscuration > 0.99);
        std::mem::swap(&mut query.first, &mut query.second);
        assert!(find_events(&engine, &scene, &query, None)?.is_empty());
        query.kind = EventKind::LunarEclipse;
        scene.date = CivilDate {
            year: 2025,
            month: 3,
            day: 14,
            hour: 0,
            minute: 0,
            second: 0.0,
        };
        scene.latitude = 37.7749;
        scene.longitude = -122.4194;
        let lunar = find_events(&engine, &scene, &query, None)?;
        assert_eq!(lunar.len(), 1);
        assert_eq!(lunar[0].r#type, "Total lunar eclipse");
        assert!((lunar[0].peak - (6.0 * 3600.0 + 58.0 * 60.0 + 41.0)).abs() < 120.0);
        let peak = engine.compute(
            &scene_at(&scene, lunar[0].peak)?,
            0,
            None,
            Scope::SolarSystem,
            None,
        )?;
        assert!(peak.lunar.solar_visibility < 0.001);
        let product = peak.lunar.fixed_from_enu * peak.lunar.fixed_from_enu.transpose();
        for i in 0..3 {
            for j in 0..3 {
                assert!((product.a[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        scene.date = CivilDate {
            year: -3000,
            month: 1,
            day: 2,
            hour: 0,
            minute: 0,
            second: 0.0,
        };
        scene.scale = TimeScale::UT1;
        let ancient = engine.compute(&scene, 0, None, Scope::SolarSystem, None)?;
        assert!(!ancient.lunar.precise_orientation && ancient.lunar.sun_km.x.is_finite());
        assert!(find_events(&engine, &scene, &query, Some(&|| true)).is_err());
        scene = Scenario {
            date: CivilDate {
                year: 2024,
                month: 3,
                day: 20,
                hour: 0,
                minute: 0,
                second: 0.0,
            },
            longitude: 0.0,
            latitude: 0.0,
            scale: TimeScale::UT1,
            atmosphere: false,
            ..Scenario::default()
        };
        let target = Target {
            body: 10,
            star: None,
        };
        let plan = observing_plan(&engine, &scene, target, None)?;
        assert_eq!(plan.samples.len(), 289);
        assert_eq!(plan.rises.len(), 1);
        assert_eq!(plan.sets.len(), 1);
        assert!((5.0 * 3600.0..7.0 * 3600.0).contains(&plan.rises[0]));
        assert!((17.0 * 3600.0..19.0 * 3600.0).contains(&plan.sets[0]));
        scene.horizon.degrees.fill(30.0);
        let ridge = observing_plan(&engine, &scene, target, None)?;
        assert!(ridge.rises[0] > plan.rises[0] && ridge.sets[0] < plan.sets[0]);
        scene.horizon = Default::default();
        scene.latitude = 90.0;
        scene.date = CivilDate {
            year: 2024,
            month: 6,
            day: 21,
            hour: 0,
            minute: 0,
            second: 0.0,
        };
        let polar = observing_plan(&engine, &scene, target, None)?;
        assert!(polar.rises.is_empty() && polar.sets.is_empty());
        assert!(observing_plan(&engine, &scene, Target::default(), Some(&|| true)).is_err());
        Ok(())
    }
}
