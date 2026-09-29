use crate::{
    eclipse::disc_overlap,
    ffi::*,
    math::{PI, RAD, angle, norm},
    photometry,
    scenario::Scenario,
    sky::{Object, Scope, SkyEngine, SkySnapshot},
    time::{TimeScale, from_jd, to_jd},
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Target {
    pub body: i32,
    pub star: Option<u32>,
}
impl Default for Target {
    fn default() -> Self {
        Self {
            body: 301,
            star: None,
        }
    }
}
pub type CancelCheck<'a> = Option<&'a dyn Fn() -> bool>;
fn check_cancel(cancelled: CancelCheck<'_>) -> Result<()> {
    if cancelled.is_some_and(|c| c()) {
        bail!("Search cancelled");
    }
    Ok(())
}
fn root(mut f: impl FnMut(f64) -> Result<f64>, mut a: f64, mut b: f64) -> Result<f64> {
    let sign = f(a)? > 0.0;
    while b - a > 0.1 {
        let middle = (a + b) / 2.0;
        if (f(middle)? > 0.0) == sign {
            a = middle;
        } else {
            b = middle;
        }
    }
    Ok((a + b) / 2.0)
}
fn minimum(mut f: impl FnMut(f64) -> Result<f64>, mut a: f64, mut b: f64) -> Result<f64> {
    const RATIO: f64 = 0.61803398874989485;
    let mut c = b - RATIO * (b - a);
    let mut d = a + RATIO * (b - a);
    let mut fc = f(c)?;
    let mut fd = f(d)?;
    while b - a > 0.1 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - RATIO * (b - a);
            fc = f(c)?;
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + RATIO * (b - a);
            fd = f(d)?;
        }
    }
    Ok((a + b) / 2.0)
}
fn sample(
    engine: &SkyEngine,
    initial: &Scenario,
    target: Target,
    seconds: f64,
    cancelled: CancelCheck<'_>,
) -> Result<SkySnapshot> {
    check_cancel(cancelled)?;
    engine.compute(
        &scene_at(initial, seconds)?,
        0,
        None,
        if target.star.is_some() {
            Scope::FullSky
        } else {
            Scope::SolarSystem
        },
        target.star,
    )
}
fn transmission(altitude: f64) -> f64 {
    if altitude <= -2.0 * RAD {
        return 0.0;
    }
    let z = PI / 2.0 - altitude.max(0.0);
    10.0f64.powf(-0.4 * 0.15 / (1.0 - 0.96 * z.sin().powi(2)).sqrt())
}

pub fn scene_at(initial: &Scenario, seconds: f64) -> Result<Scenario> {
    let mut result = initial.clone();
    if initial.scale == TimeScale::UTC && !initial.julian {
        let (mut u1, mut u2, mut a1, mut a2) = (0.0, 0.0, 0.0, 0.0);
        let d = initial.date;
        if unsafe {
            eraDtf2d(
                c"UTC".as_ptr(),
                d.year,
                d.month,
                d.day,
                d.hour,
                d.minute,
                d.second,
                &mut u1,
                &mut u2,
            )
        } < 0
            || unsafe { eraUtctai(u1, u2, &mut a1, &mut a2) } < 0
        {
            bail!("Invalid UTC date");
        }
        unsafe {
            eraTaiutc(a1, a2 + seconds / 86400.0, &mut u1, &mut u2);
        }
        let mut time = [0; 4];
        let out = &mut result.date;
        unsafe {
            eraD2dtf(
                c"UTC".as_ptr(),
                6,
                u1,
                u2,
                &mut out.year,
                &mut out.month,
                &mut out.day,
                time.as_mut_ptr(),
            );
        }
        out.hour = time[0];
        out.minute = time[1];
        out.second = time[2] as f64 + time[3] as f64 * 1e-6;
    } else {
        result.date = from_jd(
            to_jd(&initial.date, initial.julian)?.add_seconds(seconds),
            initial.julian,
        );
    }
    Ok(result)
}
pub fn target_object(sky: &SkySnapshot, target: Target) -> Result<Object> {
    let result = if let Some(index) = target.star {
        sky.stars.iter().find(|o| o.catalog_index == index)
    } else {
        sky.bodies.iter().find(|o| o.body == target.body)
    };
    result
        .copied()
        .ok_or_else(|| anyhow::anyhow!("Target unavailable for this epoch"))
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct PlanSample {
    pub seconds: f64,
    pub altitude: f64,
    pub sun_altitude: f64,
    pub moon_altitude: f64,
    pub moon_separation: f64,
    pub sky_luminance: f64,
    pub suitable: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: f64,
    pub end: f64,
}
#[derive(Clone, Debug, Default)]
pub struct ObservationPlan {
    pub initial: Scenario,
    pub target: Target,
    pub samples: Vec<PlanSample>,
    pub rises: Vec<f64>,
    pub sets: Vec<f64>,
    pub windows: Vec<TimeWindow>,
    pub transit: f64,
    pub transit_altitude: f64,
    pub best_time: f64,
    pub has_recommendation: bool,
}
pub fn observing_plan(
    engine: &SkyEngine,
    initial: &Scenario,
    target: Target,
    cancelled: CancelCheck<'_>,
) -> Result<ObservationPlan> {
    let mut plan = ObservationPlan {
        initial: initial.clone(),
        target,
        ..ObservationPlan::default()
    };
    let mut best = -1.0;
    let clearance = |t| -> Result<f64> {
        let sky = sample(engine, initial, target, t, cancelled)?;
        let o = target_object(&sky, target)?;
        Ok(o.altitude() + o.angular_radius - initial.horizon.altitude(o.azimuth()))
    };
    let mut previous_clearance = 0.0;
    for i in 0..=288 {
        let t = i as f64 * 300.0;
        let sky = sample(engine, initial, target, t, cancelled)?;
        let object = target_object(&sky, target)?;
        let moon = sky.bodies[1];
        let mut point = PlanSample {
            seconds: t,
            altitude: object.altitude(),
            sun_altitude: sky.bodies[0].geometric.z.asin(),
            moon_altitude: moon.altitude(),
            moon_separation: angle(object.observed, moon.observed),
            sky_luminance: photometry::NIGHT_FLOOR
                + photometry::pollution_luminance(initial.light_pollution),
            suitable: false,
        };
        if initial.atmosphere && initial.horizon.visible(moon.observed, 0.0) && object.body != 301 {
            point.sky_luminance += photometry::lunar_sky_luminance(
                moon.illuminance_lux,
                point.moon_separation,
                transmission(point.moon_altitude),
                transmission(point.altitude),
            );
        }
        let darkness = if object.body == 10 {
            90.0 * RAD
        } else if object.body == 301 {
            -6.0 * RAD
        } else {
            -18.0 * RAD
        };
        point.suitable = point.altitude >= 20.0 * RAD
            && point.sun_altitude <= darkness
            && initial.horizon.visible(object.observed, 0.0)
            && (object.body == 10
                || object.body == 301
                || !initial.horizon.visible(moon.observed, 0.0)
                || point.moon_separation >= 20.0 * RAD);
        if point.suitable {
            let score =
                point.altitude.sin() / (1.0 + point.sky_luminance / photometry::NIGHT_FLOOR);
            if score > best {
                best = score;
                plan.best_time = t;
                plan.has_recommendation = true;
            }
            if plan.windows.last().is_none_or(|w| w.end < t - 300.0) {
                plan.windows.push(TimeWindow { start: t, end: t });
            } else {
                plan.windows.last_mut().unwrap().end = t;
            }
        }
        plan.samples.push(point);
        let current_clearance =
            object.altitude() + object.angular_radius - initial.horizon.altitude(object.azimuth());
        if i > 0 {
            if previous_clearance < 0.0 && current_clearance >= 0.0 {
                plan.rises.push(root(clearance, t - 300.0, t)?);
            }
            if previous_clearance > 0.0 && current_clearance <= 0.0 {
                plan.sets.push(root(clearance, t - 300.0, t)?);
            }
        }
        previous_clearance = current_clearance;
    }
    let top = plan
        .samples
        .iter()
        .max_by(|a, b| a.altitude.total_cmp(&b.altitude))
        .unwrap();
    plan.transit = minimum(
        |t| Ok(-target_object(&sample(engine, initial, target, t, cancelled)?, target)?.altitude()),
        (top.seconds - 300.0).max(0.0),
        (top.seconds + 300.0).min(86400.0),
    )?;
    plan.transit_altitude = target_object(
        &sample(engine, initial, target, plan.transit, cancelled)?,
        target,
    )?
    .altitude();
    Ok(plan)
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    #[default]
    SolarEclipse,
    LunarEclipse,
    CloseApproach,
    Occultation,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SkyEvent {
    pub kind: EventKind,
    pub peak: f64,
    pub separation: f64,
    pub obscuration: f64,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub inner_start: Option<f64>,
    pub inner_end: Option<f64>,
    pub visible: bool,
    pub r#type: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventSearch {
    pub kind: EventKind,
    pub first: Target,
    pub second: Target,
    pub days: f64,
    pub maximum_separation: f64,
}
impl Default for EventSearch {
    fn default() -> Self {
        Self {
            kind: EventKind::SolarEclipse,
            first: Target {
                body: 10,
                star: None,
            },
            second: Target::default(),
            days: 35.0,
            maximum_separation: 3.0 * RAD,
        }
    }
}
#[derive(Clone, Debug, Default)]
struct Geometry {
    separation: f64,
    outer: f64,
    inner: f64,
    obscuration: f64,
    visible: bool,
    r#type: String,
}
pub fn find_events(
    engine: &SkyEngine,
    initial: &Scenario,
    request: &EventSearch,
    cancelled: CancelCheck<'_>,
) -> Result<Vec<SkyEvent>> {
    if !request.days.is_finite()
        || request.days <= 0.0
        || request.days > 370.0
        || !request.maximum_separation.is_finite()
        || request.maximum_separation <= 0.0
        || request.maximum_separation > PI
    {
        bail!("Search interval must be 0..370 days and separation 0..180 degrees");
    }
    if request.first.star.is_some() && request.second.star.is_some() {
        bail!("Choose at least one solar-system body");
    }
    let (mut a, mut b) = (request.first, request.second);
    if request.kind == EventKind::SolarEclipse {
        a = Target {
            body: 10,
            star: None,
        };
        b = Target::default();
    }
    if request.kind == EventKind::LunarEclipse {
        a = Target::default();
        b = Target {
            body: 10,
            star: None,
        };
    }
    if a.body == b.body && a.star == b.star {
        bail!("Choose two different targets");
    }
    let stellar = if a.star.is_some() { a } else { b };
    let geometry = |seconds| -> Result<Geometry> {
        let sky = sample(engine, initial, stellar, seconds, cancelled)?;
        let one = target_object(&sky, a)?;
        let two = target_object(&sky, b)?;
        let separation = angle(one.observed, two.observed);
        let mut g = Geometry {
            separation,
            outer: one.angular_radius + two.angular_radius,
            inner: (one.angular_radius - two.angular_radius).abs(),
            obscuration: if one.angular_radius > 0.0 {
                disc_overlap(one.angular_radius, two.angular_radius, separation)
            } else {
                f64::from(separation < two.angular_radius)
            },
            visible: initial.horizon.visible(one.observed, one.angular_radius),
            r#type: "Close approach".into(),
        };
        if request.kind == EventKind::SolarEclipse {
            g.r#type = if g.separation < g.inner {
                if two.angular_radius >= one.angular_radius {
                    "Total solar eclipse"
                } else {
                    "Annular solar eclipse"
                }
            } else {
                "Partial solar eclipse"
            }
            .into();
        } else if request.kind == EventKind::LunarEclipse {
            let l = sky.lunar;
            let distance = norm(l.earth_km);
            let er = (6378.137 * 1.01 / distance).asin();
            let sr = (695700.0 / norm(l.sun_km)).asin();
            let mr = (1737.4 / distance).asin();
            g.separation = angle(l.sun_km, l.earth_km);
            g.outer = er + sr + mr;
            g.inner = er - sr + mr;
            g.obscuration = 1.0 - l.solar_visibility;
            g.r#type = if g.separation < er - sr - mr {
                "Total lunar eclipse"
            } else if g.separation < g.inner {
                "Partial lunar eclipse"
            } else {
                "Penumbral lunar eclipse"
            }
            .into();
        } else if request.kind == EventKind::Occultation {
            g.r#type = "Occultation".into();
            if one.angular_radius == 0.0
                || (one.body != 0 && two.body != 0 && one.distance_au >= two.distance_au)
            {
                g.outer = -1.0;
            }
            g.obscuration = if two.angular_radius > 0.0 {
                disc_overlap(two.angular_radius, one.angular_radius, g.separation)
            } else {
                f64::from(g.separation < one.angular_radius)
            };
        }
        Ok(g)
    };
    let finish = request.days * 86400.0;
    let mut events: Vec<SkyEvent> = Vec::new();
    let mut left_time = 0.0;
    let mut middle_time = 1800.0f64.min(finish);
    let mut left = geometry(left_time)?;
    let mut middle = geometry(middle_time)?;
    let consider = |left_time: f64, right_time: f64, events: &mut Vec<SkyEvent>| -> Result<()> {
        let peak = minimum(|t| Ok(geometry(t)?.separation), left_time, right_time)?;
        if peak < 0.15
            || finish - peak < 0.15
            || events.last().is_some_and(|e| (e.peak - peak).abs() < 1.0)
        {
            return Ok(());
        }
        let g = geometry(peak)?;
        let close = request.kind == EventKind::CloseApproach;
        if g.separation
            < if close {
                request.maximum_separation
            } else {
                g.outer
            }
        {
            let mut event = SkyEvent {
                kind: request.kind,
                peak,
                separation: g.separation,
                obscuration: g.obscuration,
                visible: g.visible,
                r#type: g.r#type,
                ..SkyEvent::default()
            };
            let contact = |direction: i32, inner: bool| -> Result<Option<f64>> {
                let value = |t| -> Result<f64> {
                    let v = geometry(t)?;
                    Ok(v.separation
                        - if inner {
                            v.inner
                        } else if close {
                            request.maximum_separation
                        } else {
                            v.outer
                        })
                };
                if value(peak)? >= 0.0 {
                    return Ok(None);
                }
                let mut previous = peak;
                for i in 1..=96 {
                    let next = (peak + direction as f64 * i as f64 * 900.0).clamp(0.0, finish);
                    if value(next)? >= 0.0 {
                        return Ok(Some(root(value, previous.min(next), previous.max(next))?));
                    }
                    if next == 0.0 || next == finish {
                        return Ok(None);
                    }
                    previous = next;
                }
                Ok(None)
            };
            event.start = contact(-1, false)?;
            event.end = contact(1, false)?;
            if !close {
                event.inner_start = contact(-1, true)?;
                event.inner_end = contact(1, true)?;
            }
            if let Some(start) = event.start {
                event.visible |= geometry(start)?.visible;
            }
            if let Some(end) = event.end {
                event.visible |= geometry(end)?.visible;
            }
            events.push(event);
        }
        Ok(())
    };
    consider(0.0, middle_time, &mut events)?;
    while middle_time < finish {
        let right_time = (middle_time + 1800.0).min(finish);
        let right = geometry(right_time)?;
        if middle.separation <= left.separation && middle.separation < right.separation {
            consider(left_time, right_time, &mut events)?;
        }
        if right_time == finish {
            consider(middle_time, finish, &mut events)?;
        }
        if events.len() >= 256 {
            break;
        }
        left_time = middle_time;
        left = middle;
        middle_time = right_time;
        middle = right;
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::CivilDate;
    #[test]
    fn clock_includes_utc_leap_second() -> Result<()> {
        let scene = Scenario {
            date: CivilDate {
                year: 2016,
                month: 12,
                day: 31,
                hour: 23,
                minute: 59,
                second: 59.0,
            },
            scale: TimeScale::UTC,
            ..Scenario::default()
        };
        let leap = scene_at(&scene, 1.0)?;
        assert_eq!(leap.date.day, 31);
        assert_eq!(leap.date.second, 60.0);
        let next = scene_at(&scene, 2.0)?;
        assert_eq!(next.date.year, 2017);
        assert_eq!(next.date.day, 1);
        assert_eq!(next.date.second, 0.0);
        Ok(())
    }
}
