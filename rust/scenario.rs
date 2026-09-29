use crate::background::BACKGROUND_ID;
use crate::camera::{Camera, DEFAULT_CAMERA_FOV, ProjectionKind, maximum_fov};
use crate::horizon::Horizon;
use crate::math::RAD;
use crate::photometry::MODEL_ID;
use crate::time::{CivilDate, TimeContext, TimeScale};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub date: CivilDate,
    pub scale: TimeScale,
    pub longitude: f64,
    pub latitude: f64,
    pub height: f64,
    pub julian: bool,
    pub override_delta_t: bool,
    pub custom_delta_t: f64,
    pub azimuth: f64,
    pub elevation: f64,
    pub roll: f64,
    pub fov: f64,
    pub magnitude: f64,
    pub atmosphere: bool,
    pub ground: bool,
    pub grid: bool,
    pub labels: bool,
    pub projection: ProjectionKind,
    pub milky_way: bool,
    pub moon_surface: bool,
    pub earthshine: bool,
    pub constellations: bool,
    pub constellation_labels: bool,
    pub compare: bool,
    pub comparison_year: i32,
    pub horizon: Horizon,
    pub atmosphere_preset: i32,
    pub auto_exposure: bool,
    pub adaptive_exposure: bool,
    pub pressure: f64,
    pub temperature: f64,
    pub extinction: f64,
    pub exposure: f64,
    pub light_pollution: f64,
    pub location_name: String,
    /// Explicit migration history; migrated scenes may differ from their source appearance.
    pub migrations: Vec<String>,
}
impl Default for Scenario {
    fn default() -> Self {
        Self {
            date: CivilDate::default(),
            scale: TimeScale::LocalMean,
            longitude: 116.4074,
            latitude: 39.9042,
            height: 45.0,
            julian: false,
            override_delta_t: false,
            custom_delta_t: 69.0,
            azimuth: 180.0,
            elevation: 35.0,
            roll: 0.0,
            fov: DEFAULT_CAMERA_FOV / RAD,
            magnitude: 7.0,
            atmosphere: true,
            ground: true,
            grid: false,
            labels: true,
            projection: ProjectionKind::Perspective,
            milky_way: true,
            moon_surface: true,
            earthshine: true,
            constellations: false,
            constellation_labels: true,
            compare: false,
            comparison_year: -3000,
            horizon: Horizon::default(),
            atmosphere_preset: 0,
            auto_exposure: true,
            adaptive_exposure: false,
            pressure: 1013.25,
            temperature: 15.0,
            extinction: 0.2,
            exposure: 1.0,
            light_pollution: 0.08,
            location_name: "北京 Beijing".into(),
            migrations: Vec::new(),
        }
    }
}
impl Scenario {
    pub fn camera(&self, width: f64, height: f64) -> Camera {
        Camera {
            azimuth: self.azimuth * RAD,
            elevation: self.elevation * RAD,
            roll: self.roll * RAD,
            fov: self.fov * RAD,
            width,
            height,
            projection: self.projection,
        }
    }
}

pub fn validate(s: &Scenario) -> Result<()> {
    if !(0..=1).contains(&s.atmosphere_preset) {
        bail!("Unknown atmosphere preset");
    }
    if !(-3000..7000).contains(&s.comparison_year) {
        bail!("Invalid comparison year");
    }
    if s.horizon
        .degrees
        .iter()
        .any(|v| !v.is_finite() || !(-20.0..=89.0).contains(v))
    {
        bail!("Invalid horizon altitude");
    }
    let range = |x: f64, a: f64, b: f64| x.is_finite() && x >= a && x <= b;
    if !range(s.latitude, -90.0, 90.0)
        || !range(s.longitude, -180.0, 180.0)
        || !range(s.height, -500.0, 100000.0)
        || !range(s.fov, 1.0, maximum_fov(s.projection) / RAD)
        || !range(s.elevation, -90.0, 90.0)
        || !range(s.azimuth, 0.0, 360.0)
        || !range(s.roll, -180.0, 180.0)
        || !range(s.magnitude, -2.0, 16.0)
        || !range(s.pressure, 0.0, 1200.0)
        || !range(s.temperature, -90.0, 60.0)
        || !range(s.exposure, 2.0_f64.powi(-40), 16.0)
        || !range(s.extinction, 0.0, 2.0)
        || !range(s.light_pollution, 0.0, 1.0)
        || !range(s.custom_delta_t, -86400.0, 1e7)
    {
        bail!("Scenario contains invalid location, camera or atmosphere values");
    }
    Ok(())
}
pub fn restore_day_night(s: &mut Scenario) {
    s.atmosphere = true;
    s.auto_exposure = true;
    s.exposure = 1.0;
}

pub fn scenario_to_json(
    s: &Scenario,
    data_id: &str,
    observation: Option<&TimeContext>,
    effective_exposure: f64,
) -> Result<Value> {
    validate(s)?;
    let mut j = json!({
        "schema_version": 2, "data_id": data_id, "background_id": BACKGROUND_ID,
        "date": [s.date.year, s.date.month, s.date.day, s.date.hour, s.date.minute, s.date.second],
        "scale": s.scale as i32, "julian": s.julian, "longitude": s.longitude, "latitude": s.latitude,
        "height": s.height, "location_name": s.location_name, "override_delta_t": s.override_delta_t,
        "delta_t": s.custom_delta_t, "azimuth": s.azimuth, "elevation": s.elevation, "roll": s.roll,
        "fov": s.fov, "magnitude": s.magnitude, "atmosphere": s.atmosphere,
        "atmosphere_model": "bruneton-spectral-v1", "atmosphere_preset": s.atmosphere_preset,
        "auto_exposure": s.auto_exposure, "adaptive_exposure": s.adaptive_exposure,
        "adaptive_exposure_model": "view-trimmed-v1", "exposure_model": "hemisphere-moon-v3",
        "display_model": "colour-preserving-toe-v1", "photometry_model": MODEL_ID,
        "ground": s.ground, "grid": s.grid, "labels": s.labels, "projection": s.projection,
        "milky_way": s.milky_way, "moon_surface": s.moon_surface, "earthshine": s.earthshine,
        "constellations": s.constellations, "constellation_labels": s.constellation_labels,
        "compare": s.compare, "comparison_year": s.comparison_year,
        "horizon": { "name": s.horizon.name, "altitudes": s.horizon.degrees.as_slice() },
        "surface_model": "lro-lunar-v1", "eclipse_model": "spherical-shadow-v1",
        "pressure": s.pressure, "temperature": s.temperature, "extinction": s.extinction,
        "exposure": s.exposure, "light_pollution": s.light_pollution,
    });
    if let Some(t) = observation {
        j["observation"] = json!({ "jd_tt": [t.tt.day, t.tt.fraction], "jd_ut1": [t.ut1.day, t.ut1.fraction],
            "jd_tdb": [t.tdb.day, t.tdb.fraction], "delta_t_seconds": t.delta_t_seconds,
            "extrapolated": t.extrapolated, "time_quality": t.note, "model_version": "astra-astrometry-v1" });
    }
    if effective_exposure > 0.0 && effective_exposure.is_finite() {
        j["effective_exposure"] = json!(effective_exposure);
    }
    Ok(j)
}
pub fn save_scenario(
    s: &Scenario,
    path: impl AsRef<Path>,
    data_id: &str,
    observation: Option<&TimeContext>,
    effective_exposure: f64,
) -> Result<()> {
    let j = scenario_to_json(s, data_id, observation, effective_exposure)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(".partial");
    let temporary = std::path::PathBuf::from(name);
    std::fs::write(
        &temporary,
        format!("{}\n", serde_json::to_string_pretty(&j)?),
    )
    .context("Cannot write scenario")?;
    std::fs::rename(&temporary, path).context("Cannot write scenario")?;
    Ok(())
}
fn model(
    j: &Value,
    migrations: &mut Vec<String>,
    key: &str,
    current: &str,
    legacy: &[&str],
) -> Result<()> {
    let value = match j.get(key) {
        Some(v) => v.as_str().context("Scene model must be a string")?,
        None => "",
    };
    if value == current {
        return Ok(());
    }
    if !value.is_empty() && !legacy.contains(&value) {
        bail!("Unsupported scene model: {key}={value}");
    }
    migrations.push(format!(
        "{key}: {} -> {current}",
        if value.is_empty() { "legacy" } else { value }
    ));
    Ok(())
}
pub fn scenario_from_json(j: &Value, expected_data_id: &str) -> Result<Scenario> {
    if !matches!(j.get("schema_version").and_then(Value::as_i64), Some(1 | 2)) {
        bail!("Unsupported scenario version");
    }
    let id = j
        .get("data_id")
        .map(|v| serde_json::from_value::<String>(v.clone()))
        .transpose()?
        .unwrap_or_default();
    if !expected_data_id.is_empty() && !id.is_empty() && id != expected_data_id {
        bail!("Scenario data version differs from the installed data pack");
    }
    let background = j
        .get("background_id")
        .map(|v| serde_json::from_value::<String>(v.clone()))
        .transpose()?
        .unwrap_or_else(|| BACKGROUND_ID.into());
    if ![
        BACKGROUND_ID,
        "gaia-edr3-diffuse-v1",
        "gaia-edr3-diffuse-v2",
        "nasa-gaia-dr2-diffuse-16k-v1",
    ]
    .contains(&background.as_str())
    {
        bail!("Scenario background version differs from the installed map");
    }
    let mut s = Scenario::default();
    model(j, &mut s.migrations, "photometry_model", MODEL_ID, &[])?;
    model(
        j,
        &mut s.migrations,
        "exposure_model",
        "hemisphere-moon-v3",
        &["hemisphere-moon-v1", "hemisphere-moon-v2"],
    )?;
    model(
        j,
        &mut s.migrations,
        "display_model",
        "colour-preserving-toe-v1",
        &[],
    )?;
    if background != BACKGROUND_ID {
        s.migrations
            .push(format!("background_id: {background} -> {BACKGROUND_ID}"));
    }
    let date = j
        .get("date")
        .and_then(Value::as_array)
        .context("Invalid date tuple")?;
    if date.len() != 6 {
        bail!("Invalid date tuple");
    }
    let integer = |i: usize| serde_json::from_value::<i32>(date[i].clone());
    s.date = CivilDate {
        year: integer(0)?,
        month: integer(1)?,
        day: integer(2)?,
        hour: integer(3)?,
        minute: integer(4)?,
        second: serde_json::from_value(date[5].clone())?,
    };
    s.scale = match j.get("scale").and_then(Value::as_i64) {
        Some(0) => TimeScale::UT1,
        Some(1) => TimeScale::TT,
        Some(2) => TimeScale::TDB,
        Some(3) => TimeScale::UTC,
        Some(4) => TimeScale::LocalMean,
        _ => bail!("Invalid time scale"),
    };
    macro_rules! load { ($($name:ident),* $(,)?) => { $(if let Some(value) = j.get(stringify!($name)) { s.$name = serde_json::from_value(value.clone())?; })* }; }
    load!(
        longitude,
        latitude,
        height,
        location_name,
        override_delta_t,
        julian,
        azimuth,
        elevation,
        roll,
        fov,
        magnitude,
        atmosphere,
        atmosphere_preset,
        auto_exposure,
        adaptive_exposure,
        ground,
        grid,
        labels,
        milky_way,
        moon_surface,
        earthshine,
        constellations,
        constellation_labels,
        compare,
        comparison_year,
        pressure,
        temperature,
        extinction,
        exposure,
        light_pollution
    );
    if let Some(value) = j.get("delta_t") {
        s.custom_delta_t = serde_json::from_value(value.clone())?;
    }
    if j.get("atmosphere_model")
        .is_some_and(|v| v != "bruneton-spectral-v1")
    {
        bail!("Unknown atmosphere model");
    }
    if j.get("adaptive_exposure_model").is_some() || s.adaptive_exposure {
        model(
            j,
            &mut s.migrations,
            "adaptive_exposure_model",
            "view-trimmed-v1",
            &[],
        )?;
    }
    s.projection = if let Some(value) = j.get("projection") {
        let name = value.as_str().context("Unknown sky projection")?;
        match name {
            "perspective" => ProjectionKind::Perspective,
            "fisheye" => ProjectionKind::Fisheye,
            "stereographic" => ProjectionKind::Stereographic,
            _ => bail!("Unknown sky projection: {name}"),
        }
    } else {
        if j.get("fisheye")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or(false)
        {
            ProjectionKind::Fisheye
        } else {
            ProjectionKind::Perspective
        }
    };
    if j.get("projection").is_none() && s.fov <= 180.0 {
        s.fov = s.fov.min(maximum_fov(s.projection) / RAD);
    }
    if let Some(h) = j.get("horizon") {
        if let Some(name) = h.get("name") {
            s.horizon.name = serde_json::from_value(name.clone())?;
        }
        let altitudes: Vec<f32> = serde_json::from_value(
            h.get("altitudes")
                .context("Invalid horizon sample count")?
                .clone(),
        )?;
        s.horizon.degrees = altitudes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid horizon sample count"))?;
    }
    model(j, &mut s.migrations, "surface_model", "lro-lunar-v1", &[])?;
    model(
        j,
        &mut s.migrations,
        "eclipse_model",
        "spherical-shadow-v1",
        &[],
    )?;
    validate(&s)?;
    Ok(s)
}
pub fn load_scenario(path: impl AsRef<Path>, expected_data_id: &str) -> Result<Scenario> {
    let input = std::fs::read(path).context("Cannot open scenario")?;
    scenario_from_json(&serde_json::from_slice(&input)?, expected_data_id)
}
impl Serialize for Scenario {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        scenario_to_json(self, "", None, 0.0)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Scenario {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        scenario_from_json(&Value::deserialize(deserializer)?, "").map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_round_trip_and_exact_defaults() {
        let mut s = Scenario::default();
        s.roll = -137.25;
        s.projection = ProjectionKind::Stereographic;
        let j = scenario_to_json(&s, "test-pack", None, 0.4).unwrap();
        assert_eq!(scenario_from_json(&j, "test-pack").unwrap(), s);
        assert_eq!(j["effective_exposure"], 0.4);
        assert_eq!(j["date"], json!([2026, 9, 4, 22, 0, 0.0]));
        assert!(scenario_from_json(&j, "other-pack").is_err());
    }
    #[test]
    fn legacy_migrations_projection_and_model_rejection() {
        let mut j = json!({"schema_version":1,"date":[2026,1,1,0,0,0],"scale":0,"fisheye":false,"fov":180,"background_id":"gaia-edr3-diffuse-v1"});
        let s = scenario_from_json(&j, "").unwrap();
        assert_eq!(s.fov, 150.0);
        assert_eq!(s.migrations.len(), 6);
        assert_eq!(s.roll, 0.0);
        j["fisheye"] = json!(true);
        assert_eq!(scenario_from_json(&j, "").unwrap().fov, 180.0);
        j["projection"] = json!("stereographic");
        assert_eq!(
            scenario_from_json(&j, "").unwrap().projection,
            ProjectionKind::Stereographic
        );
        j["photometry_model"] = json!("future-v1");
        assert!(scenario_from_json(&j, "").is_err());
    }
    #[test]
    fn rejects_bad_horizon_camera_and_non_finite_values() {
        let mut s = Scenario::default();
        for invalid in [181.0, -181.0, f64::NAN] {
            s.roll = invalid;
            assert!(validate(&s).is_err());
        }
        let mut j = scenario_to_json(&Scenario::default(), "", None, 0.0).unwrap();
        j["horizon"]["altitudes"] = json!([0, 0]);
        assert!(scenario_from_json(&j, "").is_err());
        assert_eq!(crate::horizon::SAMPLE_COUNT, 720);
    }
    #[test]
    fn file_save_preserves_observation_metadata_and_replaces_atomically() {
        let path = std::env::temp_dir().join(format!(
            "astra-rust-scene-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut scene = Scenario::default();
        let time = TimeContext::default();
        save_scenario(&scene, &path, "pack-id", Some(&time), 0.25).unwrap();
        let j: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            j["observation"]["jd_tt"],
            json!([time.tt.day, time.tt.fraction])
        );
        assert_eq!(j["observation"]["model_version"], "astra-astrometry-v1");
        assert_eq!(j["effective_exposure"], 0.25);
        assert_eq!(load_scenario(&path, "pack-id").unwrap(), scene);
        scene.azimuth = 200.0;
        save_scenario(&scene, &path, "pack-id", None, 0.0).unwrap();
        assert_eq!(load_scenario(&path, "pack-id").unwrap(), scene);
        assert!(!std::path::PathBuf::from(format!("{}.partial", path.display())).exists());
        std::fs::remove_file(path).unwrap();
    }
}
