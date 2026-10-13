use crate::math::{PI, RAD, Vec3, wrap};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub const SAMPLE_COUNT: usize = 720;
#[derive(Clone, Debug, PartialEq)]
pub struct Horizon {
    pub degrees: [f32; SAMPLE_COUNT],
    pub name: String,
}
impl Default for Horizon {
    fn default() -> Self {
        Self {
            degrees: [0.0; SAMPLE_COUNT],
            name: String::new(),
        }
    }
}
impl Horizon {
    pub const SAMPLE_COUNT: usize = SAMPLE_COUNT;
    pub fn altitude(&self, azimuth: f64) -> f64 {
        let index = wrap(azimuth) * (SAMPLE_COUNT as f64 / (2.0 * PI));
        let i = index as usize % SAMPLE_COUNT;
        let fraction = index - index.floor();
        ((1.0 - fraction) * self.degrees[i] as f64
            + fraction * self.degrees[(i + 1) % SAMPLE_COUNT] as f64)
            * RAD
    }
    pub fn visible(&self, direction: Vec3, radius: f64) -> bool {
        direction.z.clamp(-1.0, 1.0).asin() + radius
            >= self.altitude(direction.x.atan2(direction.y))
    }
}
pub fn make_horizon(mut points: Vec<[f64; 2]>, name: impl Into<String>) -> Result<Horizon> {
    if !(2..=100000).contains(&points.len()) {
        bail!("A horizon needs 2 to 100000 azimuth/altitude samples");
    }
    for point in &points {
        if !point[0].is_finite()
            || !point[1].is_finite()
            || !(0.0..=360.0).contains(&point[0])
            || !(-20.0..=89.0).contains(&point[1])
        {
            bail!("Horizon azimuth must be 0..360 and altitude -20..89 degrees");
        }
    }
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then_with(|| a[1].total_cmp(&b[1])));
    if points[0][0] == 0.0 && points.last().unwrap()[0] == 360.0 {
        if (points[0][1] - points.last().unwrap()[1]).abs() > 1e-5 {
            bail!("Horizon seam samples must agree");
        }
        points.pop();
    }
    if points.windows(2).any(|p| p[0][0] == p[1][0]) {
        bail!("Duplicate horizon azimuth");
    }
    let last = *points.last().unwrap();
    points.insert(0, [last[0] - 360.0, last[1]]);
    let first = points[1];
    points.push([first[0] + 360.0, first[1]]);
    let mut out = Horizon {
        name: name.into(),
        ..Horizon::default()
    };
    let mut next = 1;
    for (i, altitude) in out.degrees.iter_mut().enumerate() {
        let az = i as f64 * 360.0 / SAMPLE_COUNT as f64;
        while points[next][0] < az {
            next += 1;
        }
        let (a, b) = (points[next - 1], points[next]);
        *altitude = (a[1] + (b[1] - a[1]) * (az - a[0]) / (b[0] - a[0])) as f32;
    }
    Ok(out)
}
pub fn load_horizon(path: impl AsRef<Path>) -> Result<Horizon> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).context("Cannot open horizon file")?;
    let mut name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let points = if path.extension().is_some_and(|s| s == "json") {
        let j: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(n) = j.get("name") {
            name = serde_json::from_value(n.clone())?;
        }
        serde_json::from_value(j.get("points").context("Missing horizon points")?.clone())?
    } else {
        let mut points = Vec::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') || line.starts_with("azimuth") {
                continue;
            }
            let line = line.replace(',', " ");
            let row: Vec<_> = line.split_whitespace().collect();
            if row.len() != 2 {
                bail!("Expected azimuth,altitude in horizon CSV");
            }
            let point = [row[0].parse::<f64>(), row[1].parse::<f64>()];
            match point {
                [Ok(az), Ok(alt)] => points.push([az, alt]),
                _ => bail!("Expected azimuth,altitude in horizon CSV"),
            }
        }
        points
    };
    make_horizon(points, name)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn periodic_interpolation_and_input_validation() {
        let h = make_horizon(vec![[180.0, 20.0], [0.0, 10.0], [360.0, 10.0]], "ridge").unwrap();
        assert!((h.altitude(90.0 * RAD) / RAD - 15.0).abs() < 1e-8);
        assert!((h.altitude(-90.0 * RAD) / RAD - 15.0).abs() < 1e-8);
        assert!(!h.visible(
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            0.0
        ));
        assert!(make_horizon(vec![[0.0, 0.0], [360.0, 1.0]], "").is_err());
        assert!(make_horizon(vec![[0.0, 1.0], [0.0, 1.0]], "").is_err());
        let flat = make_horizon(vec![[0.0, 4.0], [360.0, 4.0]], "").unwrap();
        assert!(flat.degrees.iter().all(|x| *x == 4.0));
    }
}
