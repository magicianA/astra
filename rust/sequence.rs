use crate::math::{RAD, wrap};
use crate::scenario::{Scenario, save_scenario, validate};
use crate::time::advance_date;
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SequenceRequest {
    pub directory: PathBuf,
    pub frames: i32,
    pub fps: i32,
    pub step_seconds: f64,
    pub video: bool,
    pub trails: bool,
    pub lock_exposure: bool,
    pub camera_path: bool,
    pub camera_end: Scenario,
}
impl Default for SequenceRequest {
    fn default() -> Self {
        Self {
            directory: PathBuf::new(),
            frames: 120,
            fps: 30,
            step_seconds: 60.0,
            video: true,
            trails: false,
            lock_exposure: true,
            camera_path: false,
            camera_end: Scenario::default(),
        }
    }
}
pub fn validate_sequence(r: &SequenceRequest) -> Result<()> {
    if !(2..=100000).contains(&r.frames)
        || !(1..=120).contains(&r.fps)
        || !r.step_seconds.is_finite()
        || !(0.001..=31557600.0).contains(&r.step_seconds.abs())
        || r.directory.as_os_str().is_empty()
    {
        bail!("Invalid sequence: 2..100000 frames, 1..120 fps, finite nonzero time step");
    }
    validate(&r.camera_end)
}
pub fn scene_at(initial: &Scenario, seconds: f64) -> Result<Scenario> {
    let mut result = initial.clone();
    result.date = advance_date(&initial.date, initial.scale, seconds, initial.julian)?;
    Ok(result)
}
fn remainder(x: f64, period: f64) -> f64 {
    x - (x / period).round_ties_even() * period
}
pub fn sequence_scene(start: &Scenario, r: &SequenceRequest, frame: i32) -> Result<Scenario> {
    validate_sequence(r)?;
    if !(0..r.frames).contains(&frame) {
        bail!("Invalid sequence frame");
    }
    let mut s = if frame == 0 {
        start.clone()
    } else {
        scene_at(start, r.step_seconds * frame as f64)?
    };
    if r.camera_path {
        if s.projection != r.camera_end.projection {
            bail!("Camera path needs matching projections");
        }
        if frame == 0 {
            return Ok(s);
        }
        let t = frame as f64 / (r.frames - 1) as f64;
        let t = t * t * (3.0 - 2.0 * t);
        let angular = |from: f64, to: f64| from + remainder(to - from, 360.0) * t;
        s.azimuth = wrap(angular(start.azimuth, r.camera_end.azimuth) * RAD) / RAD;
        s.elevation = start.elevation + (r.camera_end.elevation - start.elevation) * t;
        s.roll = remainder(angular(start.roll, r.camera_end.roll), 360.0);
        s.fov = (start.fov.ln() + (r.camera_end.fov.ln() - start.fov.ln()) * t).exp();
    }
    validate(&s)?;
    Ok(s)
}
pub fn save_sequence_manifest(
    r: &SequenceRequest,
    initial: &Scenario,
    completed: i32,
    cancelled: bool,
) -> Result<()> {
    let j = json!({"schema_version":1,"frames":r.frames,"completed_frames":completed,"fps":r.fps,
        "step_seconds":r.step_seconds,"video":r.video,"trails":r.trails,"cancelled":cancelled,
        "camera_path":r.camera_path,"initial_scene":"start.json","final_camera":"camera-end.json",
        "frame_pattern":"frame-%06d.png","trails_method":"maximum displayed luminance; preserves RGB; not radiometric exposure"});
    save_scenario(initial, r.directory.join("start.json"), "", None, 0.0)?;
    save_scenario(
        &r.camera_end,
        r.directory.join("camera-end.json"),
        "",
        None,
        0.0,
    )?;
    std::fs::write(
        r.directory.join("sequence.json"),
        format!("{}\n", serde_json::to_string_pretty(&j)?),
    )
    .context("Cannot save sequence manifest")?;
    Ok(())
}
/// Photograph-style lighten composite. Whole RGBA pixels preserve star colour.
pub fn accumulate_trails(
    pixels: &mut Vec<u8>,
    path: impl AsRef<Path>,
    width: &mut u32,
    height: &mut u32,
) -> Result<()> {
    let frame = image::ImageReader::open(path)?.decode()?.to_rgba8();
    let (w, h) = frame.dimensions();
    if !pixels.is_empty() && (w != *width || h != *height) {
        bail!("Window size changed during export");
    }
    *width = w;
    *height = h;
    let frame = frame.into_raw();
    if pixels.is_empty() {
        *pixels = frame;
        return Ok(());
    }
    for (dst, src) in pixels.chunks_exact_mut(4).zip(frame.chunks_exact(4)) {
        let luminance =
            |rgb: &[u8]| 0.2126 * rgb[0] as f64 + 0.7152 * rgb[1] as f64 + 0.0722 * rgb[2] as f64;
        if luminance(src) > luminance(dst) {
            dst.copy_from_slice(src);
        }
    }
    Ok(())
}
pub fn write_trails(pixels: &[u8], width: u32, height: u32, path: impl AsRef<Path>) -> Result<()> {
    if width == 0 || height == 0 || pixels.len() != width as usize * height as usize * 4 {
        bail!("Trail export failed: invalid RGBA dimensions");
    }
    image::save_buffer_with_format(
        path,
        pixels,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .context("Trail export failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ProjectionKind;
    use crate::time::{CivilDate, TimeScale};
    #[test]
    fn camera_path_shortest_angle_log_fov_and_fixed_time() {
        let start = Scenario {
            date: CivilDate {
                year: 2026,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0.0,
            },
            scale: TimeScale::UT1,
            azimuth: 350.0,
            ..Scenario::default()
        };
        let mut r = SequenceRequest {
            frames: 3,
            step_seconds: 3600.0,
            video: false,
            directory: "unused".into(),
            camera_path: true,
            camera_end: start.clone(),
            ..SequenceRequest::default()
        };
        r.camera_end.azimuth = 10.0;
        r.camera_end.fov = 15.0;
        r.camera_end.elevation = 50.0;
        assert_eq!(sequence_scene(&start, &r, 0).unwrap(), start);
        let middle = sequence_scene(&start, &r, 1).unwrap();
        let end = sequence_scene(&start, &r, 2).unwrap();
        assert_eq!(middle.date.hour, 1);
        assert_eq!(end.date.hour, 2);
        assert!(middle.azimuth.abs() < 1e-8 && (middle.fov - 30.0).abs() < 1e-8);
        assert!(
            (end.azimuth - 10.0).abs() < 1e-8
                && (end.fov - 15.0).abs() < 1e-8
                && (end.elevation - 50.0).abs() < 1e-8
        );
        r.camera_end.projection = ProjectionKind::Fisheye;
        assert!(sequence_scene(&start, &r, 0).is_err());
    }
    #[test]
    fn leap_second_sequence_keeps_physical_seconds() {
        let start = Scenario {
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
        let leap = scene_at(&start, 1.0).unwrap();
        let midnight = scene_at(&start, 2.0).unwrap();
        assert_eq!(leap.date.second, 60.0);
        assert_eq!(
            (
                midnight.date.year,
                midnight.date.month,
                midnight.date.day,
                midnight.date.hour
            ),
            (2017, 1, 1, 0)
        );
    }
}
