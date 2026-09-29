#![recursion_limit = "256"]
#[cfg(feature = "gpu")]
pub mod atmosphere;
pub mod background;
pub mod calibration;
pub mod camera;
pub mod catalog;
pub mod eclipse;
pub mod events;
#[cfg(feature = "desktop")]
pub mod explore;
pub mod ffi;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod horizon;
pub mod i18n;
pub mod image_mips;
#[cfg(feature = "gpu")]
pub mod jobs;
pub mod math;
pub mod orientation;
pub mod photometry;
pub mod recording;
pub mod render_scene;
#[cfg(feature = "gpu")]
pub mod renderer;
pub mod scenario;
pub mod sequence;
pub mod sky;
pub mod spice;
pub mod text;
pub mod time;

pub fn source_data() -> std::path::PathBuf {
    if let Some(data) = std::env::var_os("ASTRA_DATA") {
        return data.into();
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            let data = parent.join("../Resources/data");
            if data.join("catalog/manifest.json").exists() {
                return data;
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}
