use crate::ffi::*;
use crate::math::{Mat3, Vec3, cross, dot, unit};
use crate::time::TimeContext;
use anyhow::{Context, Result, ensure};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Versioned CIO integral cache. Angles are radians, time is TT days from J2000.
#[derive(Clone, Debug)]
pub struct Orientation {
    start: f64,
    step: f64,
    values: Vec<f64>,
}

fn modern_weight(year: f64) -> f64 {
    if (1900.0..=2100.0).contains(&year) {
        return 1.0;
    }
    let x = if year < 1900.0 {
        (year - 1850.0) / 50.0
    } else {
        (2150.0 - year) / 50.0
    };
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn area(a: Vec3, b: Vec3) -> f64 {
    2.0 * cross(a, b).z.atan2(1.0 + a.z + b.z + dot(a, b))
}
fn read_f64(reader: &mut impl Read) -> std::io::Result<f64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(f64::from_le_bytes(bytes))
}
impl Orientation {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let invalid =
            "Missing or invalid CIO table; run astra_cli --generate-cio DATA/time/cio.bin";
        let mut input = BufReader::new(File::open(path.as_ref()).context(invalid)?);
        let mut magic = [0; 8];
        input.read_exact(&mut magic).context(invalid)?;
        let start = read_f64(&mut input).context(invalid)?;
        let step = read_f64(&mut input).context(invalid)?;
        let mut count = [0; 8];
        input.read_exact(&mut count).context(invalid)?;
        let count = u64::from_le_bytes(count);
        ensure!(
            &magic == b"ACIO0001"
                && start.is_finite()
                && step.is_finite()
                && step > 0.0
                && (2..=10_000_000).contains(&count),
            "{invalid}"
        );
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let value = read_f64(&mut input).context("Truncated CIO table")?;
            ensure!(value.is_finite(), "Invalid CIO table value");
            values.push(value);
        }
        Ok(Self {
            start,
            step,
            values,
        })
    }

    pub fn pole(days: f64) -> Vec3 {
        if !days.is_finite() {
            return Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        }
        let year = 2000.0 + days / 365.25;
        let mut p = Mat3::default();
        let (mut equatorial, mut ecliptic) = ([0.0; 3], [0.0; 3]);
        let (mut dpsi, mut deps) = (0.0, 0.0);
        let mut n = Mat3::default();
        unsafe {
            eraLtpb(year, p.a.as_mut_ptr());
            eraLtpequ(year, equatorial.as_mut_ptr());
            eraLtpecl(year, ecliptic.as_mut_ptr());
            eraNut00a(2_451_545.0, days, &mut dpsi, &mut deps);
        }
        let eps = dot(equatorial.into(), ecliptic.into())
            .clamp(-1.0, 1.0)
            .acos();
        unsafe {
            eraNumat(eps, dpsi, deps, n.a.as_mut_ptr());
        }
        let m = n * p;
        let mut v: Vec3 = m.a[2].into();
        let weight = modern_weight(year);
        if weight > 0.0 {
            let (mut x, mut y, mut s) = (0.0, 0.0, 0.0);
            unsafe {
                eraXys06a(2_451_545.0, days, &mut x, &mut y, &mut s);
            }
            let modern = Vec3::new(x, y, (1.0 - x * x - y * y).max(0.0).sqrt());
            v = unit(v * (1.0 - weight) + modern * weight);
        }
        v
    }

    pub fn generate(path: impl AsRef<Path>, step: f64) -> Result<()> {
        ensure!(
            step.is_finite() && step > 0.0 && step <= 32.0,
            "CIO step must be (0,32] days"
        );
        let each = (5002.0 * 365.25 / step).ceil();
        ensure!(
            each < 5_000_000.0,
            "CIO step produces more than 10000000 cache entries"
        );
        let each = each as usize;
        let mut values = vec![0.0; each * 2 + 1];
        let (mut x, mut y, mut s) = (0.0, 0.0, 0.0);
        unsafe {
            eraXys06a(2_451_545.0, 0.0, &mut x, &mut y, &mut s);
        }
        values[each] = s;
        let zero = Self::pole(0.0);
        let mut a = zero;
        for i in 1..=each {
            let b = Self::pole(i as f64 * step);
            values[each + i] = values[each + i - 1] - area(a, b);
            a = b;
        }
        a = zero;
        for i in 1..=each {
            let b = Self::pole(-(i as f64) * step);
            values[each - i] = values[each - i + 1] - area(a, b);
            a = b;
        }
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut temporary = path.as_os_str().to_os_string();
        temporary.push(".partial");
        let mut output =
            BufWriter::new(File::create(&temporary).context("Writing CIO table failed")?);
        output.write_all(b"ACIO0001")?;
        output.write_all(&(-(each as f64) * step).to_le_bytes())?;
        output.write_all(&step.to_le_bytes())?;
        output.write_all(&(values.len() as u64).to_le_bytes())?;
        for value in values {
            output.write_all(&value.to_le_bytes())?;
        }
        output.flush().context("Writing CIO table failed")?;
        output.get_ref().sync_all()?;
        drop(output);
        std::fs::rename(&temporary, path).context("Publishing CIO table failed")?;
        Ok(())
    }

    pub fn cio(&self, days: f64) -> Result<f64> {
        let index = (days - self.start) / self.step;
        ensure!(
            index.is_finite() && index >= 0.0 && index < (self.values.len() - 1) as f64,
            "CIO date outside cache"
        );
        let i = index as usize;
        let w = index - i as f64;
        Ok(self.values[i] * (1.0 - w) + self.values[i + 1] * w)
    }

    pub fn celestial(&self, t: &TimeContext) -> Result<Mat3> {
        ensure!(
            t.tt.day.is_finite() && t.tt.fraction.is_finite(),
            "Non-finite TT date"
        );
        let days = t.tt.since_j2000();
        let year = 2000.0 + days / 365.25;
        let weight = modern_weight(year);
        let (mut x, mut y, mut s) = (0.0, 0.0, 0.0);
        if weight == 1.0 {
            unsafe {
                eraXys06a(t.tt.day, t.tt.fraction, &mut x, &mut y, &mut s);
            }
        } else {
            let pole = Self::pole(days);
            x = pole.x;
            y = pole.y;
            s = self.cio(days)?;
            if weight > 0.0 {
                let (mut mx, mut my, mut ms) = (0.0, 0.0, 0.0);
                unsafe {
                    eraXys06a(t.tt.day, t.tt.fraction, &mut mx, &mut my, &mut ms);
                }
                s = s * (1.0 - weight) + ms * weight;
            }
        }
        if t.eop.available {
            x += t.eop.dx;
            y += t.eop.dy;
        }
        let mut result = Mat3::default();
        unsafe {
            eraC2ixys(x, y, s, result.a.as_mut_ptr());
        }
        Ok(result)
    }

    pub fn terrestrial(&self, t: &TimeContext) -> Result<Mat3> {
        ensure!(
            t.ut1.day.is_finite() && t.ut1.fraction.is_finite(),
            "Non-finite UT1 date"
        );
        let mut result = self.celestial(t)?;
        let mut pm = Mat3::default();
        unsafe {
            eraRz(eraEra00(t.ut1.day, t.ut1.fraction), result.a.as_mut_ptr());
            let sp = if t.extrapolated {
                0.0
            } else {
                eraSp00(t.tt.day, t.tt.fraction)
            };
            eraPom00(t.eop.xp, t.eop.yp, sp, pm.a.as_mut_ptr());
        }
        Ok(pm * result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{CivilDate, EarthOrientationData, TimeScale, make_time_civil};
    fn data() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("data/time")
    }
    #[test]
    fn orientation_matches_erfa_and_stays_orthogonal() {
        let eop = EarthOrientationData::new(data().join("finals2000A.all")).unwrap();
        let orientation = Orientation::new(data().join("cio.bin")).unwrap();
        let now = make_time_civil(
            &CivilDate::new(2026, 9, 4, 12, 0, 0.0),
            TimeScale::UTC,
            0.0,
            &eop,
            false,
            0.0,
            false,
        )
        .unwrap();
        let modern = orientation.celestial(&now).unwrap();
        let (mut x, mut y, mut s) = (0.0, 0.0, 0.0);
        let mut reference = Mat3::default();
        unsafe {
            eraXys06a(now.tt.day, now.tt.fraction, &mut x, &mut y, &mut s);
            eraC2ixys(x + now.eop.dx, y + now.eop.dy, s, reference.a.as_mut_ptr());
        }
        for i in 0..3 {
            for j in 0..3 {
                assert!((modern.a[i][j] - reference.a[i][j]).abs() < 1e-12);
            }
        }
        for year in (-3000..7000).step_by(100) {
            let t = make_time_civil(
                &CivilDate::new(year, 7, 1, 0, 0, 0.0),
                TimeScale::UT1,
                0.0,
                &eop,
                false,
                0.0,
                false,
            )
            .unwrap();
            let r = orientation.terrestrial(&t).unwrap();
            let identity = r * r.transpose();
            for i in 0..3 {
                for j in 0..3 {
                    assert!(
                        (identity.a[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12,
                        "year {year}"
                    );
                }
            }
        }
        assert!(orientation.cio(f64::NAN).is_err());
        assert!(Orientation::generate("unused.bin", 0.0).is_err());
    }
    #[test]
    fn rejects_truncated_or_invalid_cio_tables() {
        let path =
            std::env::temp_dir().join(format!("astra-cio-invalid-{}.bin", std::process::id()));
        std::fs::write(&path, b"ACIO0001").unwrap();
        assert!(Orientation::new(&path).is_err());
        let mut bytes = b"ACIO0001".to_vec();
        bytes.extend(0.0_f64.to_le_bytes());
        bytes.extend(4.0_f64.to_le_bytes());
        bytes.extend(2_u64.to_le_bytes());
        bytes.extend(0.0_f64.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(Orientation::new(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn generated_cache_retains_anchor_and_full_date_coverage() {
        let path =
            std::env::temp_dir().join(format!("astra-cio-generated-{}.bin", std::process::id()));
        Orientation::generate(&path, 32.0).unwrap();
        let orientation = Orientation::new(&path).unwrap();
        let (mut x, mut y, mut s) = (0.0, 0.0, 0.0);
        unsafe {
            eraXys06a(2_451_545.0, 0.0, &mut x, &mut y, &mut s);
        }
        assert!((orientation.cio(0.0).unwrap() - s).abs() < 1e-15);
        assert!(orientation.cio(-5000.0 * 365.25).unwrap().is_finite());
        assert!(orientation.cio(5000.0 * 365.25).unwrap().is_finite());
        std::fs::remove_file(path).unwrap();
    }
}
