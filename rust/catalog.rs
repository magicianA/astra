use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

pub const RV_UNKNOWN: u32 = 1;
pub const DISTANCE_UNKNOWN: u32 = 2;
pub const PM_UNKNOWN: u32 = 4;
pub const MULTIPLE: u32 = 8;
pub const GAIA: u32 = 16;
pub const LOW_QUALITY: u32 = 32;
pub const PROPAGATION_WARNING: u32 = 64;
pub const POSITION_MATCH: u32 = 128;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct Star {
    pub id: u64,
    pub hip: u32,
    pub flags: u32,
    pub ra: f64,
    pub dec: f64,
    pub pmra: f64,
    pub pmdec: f64,
    pub parallax: f64,
    pub rv: f64,
    pub epoch: f64,
    pub magnitude: f32,
    pub bv: f32,
    pub ra_error: f32,
    pub dec_error: f32,
    pub pmra_error: f32,
    pub pmdec_error: f32,
}
const _: () = assert!(std::mem::size_of::<Star>() == 96);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Constellation {
    pub id: String,
    pub name: String,
    pub segments: Vec<[u32; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    names: HashMap<u32, String>,
    id_order: Vec<u32>,
    hip_order: Vec<(u32, u32)>,
    search_names: Vec<(String, u32)>,
    pub stars: Vec<Star>,
    pub constellations: Vec<Constellation>,
    pub constellation_member: Vec<bool>,
    pub data_id: String,
}

fn ascii_lower(text: &str) -> String {
    text.chars().map(|c| c.to_ascii_lowercase()).collect()
}
fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path).with_context(
        || format!("Cannot read {}", path.display()),
    )?)?)
}

impl Catalog {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let pack = fs::read(root.join("catalog/stars.bin"))
            .context("Missing/invalid star pack; run scripts/build_catalog.py")?;
        if pack.len() < 24 || &pack[..8] != b"ASTARS01" {
            bail!("Missing/invalid star pack; run scripts/build_catalog.py");
        }
        let count = u64::from_le_bytes(pack[8..16].try_into().unwrap());
        let size = u64::from_le_bytes(pack[16..24].try_into().unwrap());
        if size != 96 || !(1..=10_000_000).contains(&count) {
            bail!("Missing/invalid star pack; run scripts/build_catalog.py");
        }
        if pack.len() < 24 + count as usize * 96 {
            bail!("Truncated star pack");
        }
        let mut stars = Vec::with_capacity(count as usize);
        for p in pack[24..24 + count as usize * 96].chunks_exact(96) {
            let d = |i| f64::from_le_bytes(p[i..i + 8].try_into().unwrap());
            let f = |i| f32::from_le_bytes(p[i..i + 4].try_into().unwrap());
            stars.push(Star {
                id: u64::from_le_bytes(p[..8].try_into().unwrap()),
                hip: u32::from_le_bytes(p[8..12].try_into().unwrap()),
                flags: u32::from_le_bytes(p[12..16].try_into().unwrap()),
                ra: d(16),
                dec: d(24),
                pmra: d(32),
                pmdec: d(40),
                parallax: d(48),
                rv: d(56),
                epoch: d(64),
                magnitude: f(72),
                bv: f(76),
                ra_error: f(80),
                dec_error: f(84),
                pmra_error: f(88),
                pmdec_error: f(92),
            });
        }
        let mut catalog = Self {
            stars,
            ..Self::default()
        };
        let names = root.join("catalog/names.json");
        if names.exists() {
            for (hip, value) in read_json(&names)?
                .as_object()
                .context("Invalid catalogue names")?
            {
                catalog.names.insert(
                    hip.parse()?,
                    value["name"].as_str().unwrap_or("").to_owned(),
                );
            }
        }
        catalog.data_id = read_json(&root.join("catalog/manifest.json"))?["data_id"]
            .as_str()
            .context("Star pack manifest missing data_id")?
            .to_owned();
        for (i, star) in catalog.stars.iter().enumerate() {
            if star.flags & GAIA != 0 {
                catalog.id_order.push(i as u32);
            }
            if star.hip != 0 {
                catalog.hip_order.push((star.hip, i as u32));
            }
            if let Some(name) = catalog.names.get(&star.hip) {
                catalog.search_names.push((ascii_lower(name), i as u32));
            }
        }
        catalog
            .id_order
            .sort_unstable_by_key(|&i| catalog.stars[i as usize].id);
        catalog.hip_order.sort_unstable();
        catalog.constellation_member = vec![false; catalog.stars.len()];
        let figures = read_json(&root.join("skycultures/western.json"))
            .context("Missing constellation data; run scripts/prepare_exploration.py")?;
        for item in figures["figures"]
            .as_array()
            .context("Invalid constellation data")?
        {
            let mut figure = Constellation {
                id: item["id"]
                    .as_str()
                    .context("Missing constellation id")?
                    .to_owned(),
                name: item["name"]
                    .as_str()
                    .context("Missing constellation name")?
                    .to_owned(),
                segments: Vec::new(),
            };
            for line in item["lines"]
                .as_array()
                .context("Missing constellation lines")?
            {
                let line = line.as_array().context("Invalid constellation line")?;
                for pair in line.windows(2) {
                    let a = u32::try_from(pair[0].as_u64().context("Invalid HIP vertex")?)?;
                    let b = u32::try_from(pair[1].as_u64().context("Invalid HIP vertex")?)?;
                    if let (Some(a), Some(b)) = (catalog.hip_index(a), catalog.hip_index(b)) {
                        catalog.constellation_member[a as usize] = true;
                        catalog.constellation_member[b as usize] = true;
                        figure.segments.push([a, b]);
                    }
                }
            }
            catalog.constellations.push(figure);
        }
        Ok(catalog)
    }
    pub fn hip_index(&self, hip: u32) -> Option<u32> {
        let i = self.hip_order.partition_point(|&(h, _)| h < hip);
        self.hip_order
            .get(i)
            .filter(|&&(h, _)| h == hip)
            .map(|&(_, i)| i)
    }
    pub fn search(&self, query: impl AsRef<str>, limit: usize) -> Vec<u32> {
        if limit == 0 {
            return Vec::new();
        }
        let query = ascii_lower(query.as_ref());
        let query = query.trim_matches([' ', '\t']);
        if query.is_empty() {
            return Vec::new();
        }
        let gaia = query.starts_with("gaia");
        let mut digits = query;
        for prefix in ["gaia dr3", "gaia", "hip"] {
            if let Some(rest) = digits.strip_prefix(prefix) {
                digits = rest;
                break;
            }
        }
        digits = digits.trim_start_matches(' ');
        if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
            if let Ok(number) = digits.parse::<u64>() {
                if !gaia && number <= u32::MAX as u64 {
                    if let Some(index) = self.hip_index(number as u32) {
                        return vec![index];
                    }
                }
                let i = self
                    .id_order
                    .partition_point(|&i| self.stars[i as usize].id < number);
                return self
                    .id_order
                    .get(i)
                    .filter(|&&i| self.stars[i as usize].id == number)
                    .copied()
                    .into_iter()
                    .collect();
            }
        }
        self.search_names
            .iter()
            .filter(|(name, _)| name.contains(query))
            .take(limit)
            .map(|&(_, i)| i)
            .collect()
    }
    pub fn name(&self, star: &Star) -> String {
        self.names
            .get(&star.hip)
            .cloned()
            .unwrap_or_else(|| self.id(star))
    }
    pub fn id(&self, star: &Star) -> String {
        if star.flags & GAIA != 0 {
            format!("Gaia DR3 {}", star.id)
        } else {
            format!("HIP {}", star.hip)
        }
    }
}

pub fn star_color(bv: f64) -> [f32; 3] {
    let temp = (4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62)))
        .clamp(2000.0, 30000.0)
        / 100.0;
    let r = if temp <= 66.0 {
        255.0
    } else {
        329.698727446 * (temp - 60.0).powf(-0.1332047592)
    };
    let g = if temp <= 66.0 {
        99.4708025861 * temp.ln() - 161.1195681661
    } else {
        288.1221695283 * (temp - 60.0).powf(-0.0755148492)
    };
    let b = if temp >= 66.0 {
        255.0
    } else if temp <= 19.0 {
        0.0
    } else {
        138.5177312231 * (temp - 10.0).ln() - 305.0447927307
    };
    [
        (r / 255.0).clamp(0.0, 1.0) as f32,
        (g / 255.0).clamp(0.0, 1.0) as f32,
        (b / 255.0).clamp(0.0, 1.0) as f32,
    ]
}
