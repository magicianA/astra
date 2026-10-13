use anyhow::{Context, Result, bail, ensure};
use astro::{
    events::*, horizon::load_horizon, math::*, orientation::Orientation, scenario::*, sky::*,
    time::*,
};
use serde_json::{Value, json};
use std::path::PathBuf;
fn parse_date(text: &str, mut d: CivilDate) -> Result<CivilDate> {
    let (date, time) = text
        .split_once('T')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let p: Vec<_> = date.rsplitn(3, '-').collect();
    ensure!(p.len() == 3, "Date format: -3000-01-01T22:00:00");
    d.year = p[2].parse()?;
    d.month = p[1].parse()?;
    d.day = p[0].parse()?;
    if let Some(time) = time {
        let p: Vec<_> = time.split(':').collect();
        ensure!(p.len() == 3, "Time format: HH:MM:SS");
        d.hour = p[0].parse()?;
        d.minute = p[1].parse()?;
        d.second = p[2].parse()?;
    }
    let date_only = CivilDate {
        second: d.second.min(59.0),
        ..d
    };
    ensure!(
        valid_date(&date_only, false) && d.second.is_finite() && (0.0..=60.0).contains(&d.second),
        "Invalid date"
    );
    Ok(d)
}
fn item(o: &Object) -> Value {
    json!({"id":o.id.to_string(),"hip":o.hip,"body":o.body,"azimuth_deg":o.azimuth()/RAD,"altitude_deg":o.altitude()/RAD,"geometric_altitude_deg":o.geometric.z.asin()/RAD,"ra_icrs_axes_deg":wrap(o.icrs.y.atan2(o.icrs.x))/RAD,"dec_icrs_axes_deg":o.icrs.z.asin()/RAD,"magnitude":o.magnitude,"illuminance_above_atmosphere_lux":o.illuminance_lux,"angular_radius_deg":o.angular_radius/RAD,"distance_au":o.distance_au,"phase":o.phase,"quality":quality_text(o)})
}
fn write_json(output: Option<PathBuf>, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)? + "\n";
    if let Some(path) = output {
        std::fs::write(path, text).context("Cannot write output")?;
    } else {
        println!("{text}");
    }
    Ok(())
}
fn run() -> Result<()> {
    let mut data = astro::source_data();
    let mut s = Scenario {
        scale: TimeScale::UT1,
        ..Default::default()
    };
    let mut output = None;
    let mut geometric = false;
    let mut twilight = 0i32;
    let mut plan = false;
    let mut search = None;
    let mut first = Target {
        body: 301,
        star: None,
    };
    let mut second = Target {
        body: 10,
        star: None,
    };
    let mut hip = None;
    let mut days = 35.;
    let mut separation = 3.;
    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("Missing value for {arg}"))
        };
        match arg.as_str() {
            "--generate-cio" => {
                let path = value()?;
                let step = if let Some(v) = args.next() {
                    v.parse()?
                } else {
                    4.
                };
                Orientation::generate(&path, step)?;
                println!("CIO table written: {path} step={step} days");
                return Ok(());
            }
            "--compare-cio" => {
                let x = Orientation::new(value()?)?;
                let y = Orientation::new(value()?)?;
                let mut worst = 0f64;
                for k in -5000..=5000 {
                    let days = k as f64 * 365.25 + 0.3;
                    worst = worst.max((x.cio(days)? - y.cio(days)?).abs() / ARCSEC);
                }
                println!("Maximum CIO difference: {worst:.12} arcsec");
                ensure!(worst < 0.05, "CIO difference exceeds 0.05 arcsec");
                return Ok(());
            }
            "--plan" => plan = true,
            "--events" => {
                let kind = match value()?.as_str() {
                    "solar" => EventKind::SolarEclipse,
                    "lunar" => EventKind::LunarEclipse,
                    "approach" => EventKind::CloseApproach,
                    "occultation" => EventKind::Occultation,
                    _ => bail!("Event type: solar|lunar|approach|occultation"),
                };
                search = Some(EventSearch {
                    kind,
                    ..Default::default()
                });
            }
            "--target" => first.body = value()?.parse()?,
            "--second" => second.body = value()?.parse()?,
            "--hip" => hip = Some(value()?.parse::<u32>()?),
            "--days" => days = value()?.parse()?,
            "--separation" => separation = value()?.parse()?,
            "--horizon" => s.horizon = load_horizon(value()?)?,
            "--data" => data = value()?.into(),
            "--date" => s.date = parse_date(&value()?, s.date)?,
            "--site" => {
                let text = value()?;
                let p: Vec<_> = text.split(',').collect();
                ensure!(
                    p.len() == 2 || p.len() == 3,
                    "Site: longitude,latitude,height"
                );
                s.longitude = p[0].parse()?;
                s.latitude = p[1].parse()?;
                if p.len() == 3 {
                    s.height = p[2].parse()?;
                }
            }
            "--scale" => {
                s.scale = match value()?.as_str() {
                    "UT1" => TimeScale::UT1,
                    "TT" => TimeScale::TT,
                    "TDB" => TimeScale::TDB,
                    "UTC" => TimeScale::UTC,
                    "LMT" => TimeScale::LocalMean,
                    _ => bail!("Unknown time scale"),
                }
            }
            "--magnitude" => s.magnitude = value()?.parse()?,
            "--delta-t" => {
                s.override_delta_t = true;
                s.custom_delta_t = value()?.parse()?;
            }
            "--no-atmosphere" => s.atmosphere = false,
            "--output" => output = Some(PathBuf::from(value()?)),
            "--geometric" => geometric = true,
            "--twilight" => {
                twilight = match value()?.as_str() {
                    "next-dawn" => 1,
                    "previous-dawn" => -1,
                    "next-dusk" => 2,
                    "previous-dusk" => -2,
                    _ => bail!("Unknown twilight event"),
                }
            }
            "--scenario" => s = load_scenario(value()?, "")?,
            "--help" | "-h" => {
                println!(
                    "astra_cli --data DIR --date YEAR-MM-DDTHH:MM:SS --site LON,LAT,HEIGHT --scale UT1|TT|TDB|UTC|LMT --magnitude N --delta-t SECONDS --no-atmosphere --output result.json --geometric --scenario FILE\n--twilight next-dawn|previous-dawn|next-dusk|previous-dusk\n--plan | --events solar|lunar|approach|occultation --days N --target NAIF_ID --second NAIF_ID --hip HIP_ID --separation DEGREES --horizon FILE\n--generate-cio FILE [STEP_DAYS] | --compare-cio FIRST SECOND"
                );
                return Ok(());
            }
            _ => bail!("Unknown argument: {arg}"),
        }
    }
    ensure!(
        u32::from(plan) + u32::from(search.is_some()) + u32::from(twilight != 0) <= 1,
        "Choose one of plan, events or twilight"
    );
    validate(&s)?;
    for migration in &s.migrations {
        eprintln!("Scene migration: {migration}");
    }
    let engine = SkyEngine::new(&data)?;
    if twilight != 0 {
        let event = engine
            .twilight_view(&s, twilight.abs() == 1, twilight.signum(), 0, None)?
            .context("No twilight event within 370 days or the data range")?;
        save_scenario(
            &event,
            output.context("--twilight requires --output SCENARIO.json")?,
            "",
            None,
            0.,
        )?;
        return Ok(());
    }
    if plan || search.is_some() {
        if let Some(hip) = hip {
            let target = Target {
                body: 0,
                star: Some(
                    engine
                        .catalog
                        .hip_index(hip)
                        .context("HIP target is absent from this catalog")?,
                ),
            };
            if plan {
                first = target;
            } else {
                second = target;
            }
        }
        let mut report = json!({"date":format_date(&s.date),"time_scale":scale_name(s.scale),"site":[s.longitude,s.latitude,s.height],"data_id":engine.catalog.data_id,"model":"spherical-shadow-v1","time_unit":"seconds from date"});
        if plan {
            let result = observing_plan(&engine, &s, first, None)?;
            report["samples"]=json!(result.samples.iter().map(|p|json!({"seconds":p.seconds,"altitude_deg":p.altitude/RAD,"sun_altitude_deg":p.sun_altitude/RAD,"moon_altitude_deg":p.moon_altitude/RAD,"moon_separation_deg":p.moon_separation/RAD,"sky_cd_m2":p.sky_luminance,"suitable":p.suitable})).collect::<Vec<_>>());
            report["rises"] = json!(result.rises);
            report["sets"] = json!(result.sets);
            report["transit"] = json!(result.transit);
            report["transit_altitude_deg"] = json!(result.transit_altitude / RAD);
            report["recommended"] = json!(result.has_recommendation);
            report["best_time"] = json!(result.best_time);
            report["windows"] = json!(
                result
                    .windows
                    .iter()
                    .map(|w| [w.start, w.end])
                    .collect::<Vec<_>>()
            );
        } else {
            let mut search = search.unwrap();
            search.first = first;
            search.second = second;
            search.days = days;
            search.maximum_separation = separation * RAD;
            let events = find_events(&engine, &s, &search, None)?;
            report["events"]=json!(events.iter().map(|e|Ok(json!({"type":e.r#type,"peak":e.peak,"peak_date":format_date(&scene_at(&s,e.peak)?.date),"separation_deg":e.separation/RAD,"obscuration":e.obscuration,"visible":e.visible,"start":e.start,"end":e.end,"inner_start":e.inner_start,"inner_end":e.inner_end}))).collect::<Result<Vec<_>>>()?);
        }
        return write_json(output, &report);
    }
    let mut sky = engine.compute(&s, 0, None, Scope::FullSky, None)?;
    let mut j = json!({"data_id":sky.data_id,"date":format_date(&s.date),"input_scale":scale_name(s.scale),"jd_ut1":sky.time.ut1.value(),"jd_tt":sky.time.tt.value(),"jd_tdb":sky.time.tdb.value(),"delta_t_seconds":sky.time.delta_t_seconds,"quality":sky.time.note,"catalog_count":sky.catalog_count,"computed_stars":sky.stars.len(),"compute_ms":sky.compute_ms,"solar_visibility":sky.solar_visibility,"lunar_solar_visibility":sky.lunar.solar_visibility,"lunar_precise_orientation":sky.lunar.precise_orientation,"bodies":[],"bright_stars":[]});
    let ephemeris = if geometric {
        Some(Ephemeris::new(&data)?)
    } else {
        None
    };
    let mut bodies = Vec::new();
    for o in &sky.bodies {
        let mut v = item(o);
        v["name"] = json!(body_name(o.body));
        if let Some(e) = &ephemeris {
            let st = e.state(o.body, sky.time.tdb)?;
            v["barycentric_km"] = json!([st.position.x, st.position.y, st.position.z]);
        }
        bodies.push(v);
    }
    j["bodies"] = json!(bodies);
    sky.stars
        .sort_by(|a, b| a.magnitude.total_cmp(&b.magnitude));
    j["bright_stars"] = json!(
        sky.stars
            .iter()
            .take(25)
            .map(|o| {
                let mut v = item(o);
                v["name"] = json!(
                    engine
                        .catalog
                        .name(&engine.catalog.stars[o.catalog_index as usize])
                );
                v
            })
            .collect::<Vec<_>>()
    );
    write_json(output, &j)
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Astra: {e:#}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_dates_and_partial_input() {
        let d = parse_date("-3000-01-01T22:00:00", CivilDate::default()).unwrap();
        assert_eq!(d.year, -3000);
        assert_eq!(parse_date("2026-09-04", d).unwrap().hour, 22);
        assert!(parse_date("2026-02-30", d).is_err());
    }
}
