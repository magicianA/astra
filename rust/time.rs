use crate::ffi::*;
use crate::math::{ARCSEC, RAD};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeScale {
    #[default]
    UT1,
    TT,
    TDB,
    UTC,
    LocalMean,
}

/// A split Julian date retains subsecond precision across the full date range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct JulianDate {
    pub day: f64,
    pub fraction: f64,
}
impl Default for JulianDate {
    fn default() -> Self {
        Self {
            day: 2_451_544.5,
            fraction: 0.5,
        }
    }
}
impl JulianDate {
    pub fn from(jd: f64) -> Self {
        let day = (jd - 0.5).floor() + 0.5;
        Self {
            day,
            fraction: jd - day,
        }
    }
    pub fn add_seconds(self, seconds: f64) -> Self {
        let f = self.fraction + seconds / 86400.0;
        let whole = f.floor();
        Self {
            day: self.day + whole,
            fraction: f - whole,
        }
    }
    pub fn value(self) -> f64 {
        self.day + self.fraction
    }
    pub fn since_j2000(self) -> f64 {
        (self.day - 2_451_545.0) + self.fraction
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CivilDate {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
    pub second: f64,
}
impl Default for CivilDate {
    fn default() -> Self {
        Self {
            year: 2026,
            month: 9,
            day: 4,
            hour: 22,
            minute: 0,
            second: 0.0,
        }
    }
}
impl CivilDate {
    pub const fn new(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: f64) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}
fn leap(year: i32, julian: bool) -> bool {
    year % 4 == 0 && (julian || year % 100 != 0 || year % 400 == 0)
}
pub fn valid_date(c: &CivilDate, julian: bool) -> bool {
    let days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (-4799..=9999).contains(&c.year)
        && (1..=12).contains(&c.month)
        && c.day >= 1
        && c.day <= days[(c.month - 1) as usize] + i32::from(c.month == 2 && leap(c.year, julian))
        && (0..24).contains(&c.hour)
        && (0..60).contains(&c.minute)
        && c.second.is_finite()
        && (0.0..60.0).contains(&c.second)
}
pub fn to_jd(c: &CivilDate, julian: bool) -> Result<JulianDate> {
    ensure!(valid_date(c, julian), "Invalid calendar date or clock time");
    let (mut y, mut m) = (c.year, c.month);
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let correction = if julian {
        0.0
    } else {
        2.0 - (y as f64 / 100.0).floor() + (y as f64 / 400.0).floor()
    };
    Ok(JulianDate {
        day: (365.25 * (y + 4716) as f64).floor()
            + (30.6001 * (m + 1) as f64).floor()
            + c.day as f64
            + correction
            - 1524.5,
        fraction: (c.hour as f64 * 3600.0 + c.minute as f64 * 60.0 + c.second) / 86400.0,
    })
}
pub fn from_jd(jd: JulianDate, julian: bool) -> CivilDate {
    let z = (jd.day + jd.fraction + 0.5).floor();
    let f = (jd.day - (z - 0.5)) + jd.fraction;
    let a = if julian {
        z
    } else {
        let alpha = ((z - 1_867_216.25) / 36524.25).floor();
        z + 1.0 + alpha - (alpha / 4.0).floor()
    };
    let b = a + 1524.0;
    let c = ((b - 122.1) / 365.25).floor();
    let d = (365.25 * c).floor();
    let e = ((b - d) / 30.6001).floor();
    let day = (b - d - (30.6001 * e).floor()) as i32;
    let month = if e < 14.0 { e - 1.0 } else { e - 13.0 } as i32;
    let year = if month > 2 { c - 4716.0 } else { c - 4715.0 } as i32;
    let mut secs = (f * 86400.0).max(0.0);
    let hour = (secs / 3600.0) as i32;
    secs -= hour as f64 * 3600.0;
    let minute = (secs / 60.0) as i32;
    CivilDate {
        year,
        month,
        day,
        hour,
        minute,
        second: secs - minute as f64 * 60.0,
    }
}
pub fn format_date(c: &CivilDate) -> String {
    let era = if c.year <= 0 {
        format!("BCE {}", 1 - c.year)
    } else {
        format!("CE {}", c.year)
    };
    format!(
        "{era}-{:02}-{:02}  {:02}:{:02}:{:02}",
        c.month, c.day, c.hour, c.minute, c.second as i32
    )
}
pub fn decimal_year(jd: JulianDate) -> f64 {
    let c = from_jd(jd, false);
    let Ok(start) = to_jd(&CivilDate::new(c.year, 1, 1, 0, 0, 0.0), false) else {
        return f64::NAN;
    };
    c.year as f64
        + ((jd.day - start.day) + jd.fraction) / if leap(c.year, false) { 366.0 } else { 365.0 }
}

/// Espenak and Meeus, NASA Five Millennium Canon. Remote epochs are nominal
/// extrapolations, as in the original application.
pub fn delta_t(y: f64) -> f64 {
    if !(-500.0..2150.0).contains(&y) {
        let u = (y - 1820.0) / 100.0;
        return -20.0 + 32.0 * u * u;
    }
    if y < 500.0 {
        let u = y / 100.0;
        return 10583.6 - 1014.41 * u + 33.78311 * u.powi(2)
            - 5.952053 * u.powi(3)
            - 0.1798452 * u.powi(4)
            + 0.022174192 * u.powi(5)
            + 0.0090316521 * u.powi(6);
    }
    if y < 1600.0 {
        let u = (y - 1000.0) / 100.0;
        return 1574.2 - 556.01 * u + 71.23472 * u * u + 0.319781 * u.powi(3)
            - 0.8503463 * u.powi(4)
            - 0.005050998 * u.powi(5)
            + 0.0083572073 * u.powi(6);
    }
    if y < 1700.0 {
        let t = y - 1600.0;
        return 120.0 - 0.9808 * t - 0.01532 * t * t + t.powi(3) / 7129.0;
    }
    if y < 1800.0 {
        let t = y - 1700.0;
        return 8.83 + 0.1603 * t - 0.0059285 * t * t + 0.00013336 * t.powi(3)
            - t.powi(4) / 1174000.0;
    }
    if y < 1860.0 {
        let t = y - 1800.0;
        return 13.72 - 0.332447 * t + 0.0068612 * t * t + 0.0041116 * t.powi(3)
            - 0.00037436 * t.powi(4)
            + 0.0000121272 * t.powi(5)
            - 0.0000001699 * t.powi(6)
            + 0.000000000875 * t.powi(7);
    }
    if y < 1900.0 {
        let t = y - 1860.0;
        return 7.62 + 0.5737 * t - 0.251754 * t * t + 0.01680668 * t.powi(3)
            - 0.0004473624 * t.powi(4)
            + t.powi(5) / 233174.0;
    }
    if y < 1920.0 {
        let t = y - 1900.0;
        return -2.79 + 1.494119 * t - 0.0598939 * t * t + 0.0061966 * t.powi(3)
            - 0.000197 * t.powi(4);
    }
    if y < 1941.0 {
        let t = y - 1920.0;
        return 21.20 + 0.84493 * t - 0.076100 * t * t + 0.0020936 * t.powi(3);
    }
    if y < 1961.0 {
        let t = y - 1950.0;
        return 29.07 + 0.407 * t - t * t / 233.0 + t.powi(3) / 2547.0;
    }
    if y < 1986.0 {
        let t = y - 1975.0;
        return 45.45 + 1.067 * t - t * t / 260.0 - t.powi(3) / 718.0;
    }
    if y < 2005.0 {
        let t = y - 2000.0;
        return 63.86 + 0.3345 * t - 0.060374 * t * t
            + 0.0017275 * t.powi(3)
            + 0.000651814 * t.powi(4)
            + 0.00002373599 * t.powi(5);
    }
    if y < 2050.0 {
        let t = y - 2000.0;
        return 62.92 + 0.32217 * t + 0.005589 * t * t;
    }
    let u = (y - 1820.0) / 100.0;
    -20.0 + 32.0 * u * u - 0.5628 * (2150.0 - y)
}
pub fn scale_name(scale: TimeScale) -> &'static str {
    match scale {
        TimeScale::UT1 => "UT1",
        TimeScale::TT => "TT",
        TimeScale::TDB => "TDB",
        TimeScale::UTC => "UTC",
        TimeScale::LocalMean => "LMT",
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Eop {
    pub mjd: f64,
    pub xp: f64,
    pub yp: f64,
    pub dut1: f64,
    pub dx: f64,
    pub dy: f64,
    pub available: bool,
    pub predicted: bool,
}
#[derive(Clone, Debug, Default)]
pub struct EarthOrientationData {
    rows: Vec<Eop>,
}
fn field(line: &str, start: usize, count: usize) -> Option<f64> {
    line.get(start..start + count)?
        .trim()
        .parse()
        .ok()
        .filter(|x: &f64| x.is_finite())
}
fn dat(jd: JulianDate) -> f64 {
    let c = from_jd(jd, false);
    let mut result = 0.0;
    unsafe {
        eraDat(c.year, c.month, c.day, jd.fraction, &mut result);
    }
    result
}
impl EarthOrientationData {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let data = match std::fs::read_to_string(path.as_ref()) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error).context("Reading Earth orientation data"),
        };
        let mut rows = Vec::new();
        for line in data.lines().filter(|line| line.len() >= 125) {
            let Some(mjd) = field(line, 7, 8) else {
                continue;
            };
            let Some(xp) = field(line, 18, 9) else {
                continue;
            };
            let Some(yp) = field(line, 37, 9) else {
                continue;
            };
            let Some(dut1) = field(line, 58, 10) else {
                continue;
            };
            let mut row = Eop {
                mjd,
                xp: xp * ARCSEC,
                yp: yp * ARCSEC,
                dut1,
                available: true,
                predicted: line.as_bytes()[16] == b'P' || line.as_bytes()[57] == b'P',
                ..Eop::default()
            };
            if let Some(dx) = field(line, 97, 9) {
                row.dx = dx * ARCSEC / 1000.0;
                if let Some(dy) = field(line, 116, 9) {
                    row.dy = dy * ARCSEC / 1000.0;
                }
            }
            rows.push(row);
        }
        Ok(Self { rows })
    }
    pub fn last_mjd(&self) -> f64 {
        self.rows.last().map_or(0.0, |r| r.mjd)
    }
    pub fn at(&self, utc: JulianDate) -> Eop {
        let m = (utc.day - 2_400_000.5) + utc.fraction;
        if !m.is_finite() || self.rows.is_empty() || m < self.rows[0].mjd || m > self.last_mjd() {
            return Eop::default();
        }
        let index = self.rows.partition_point(|row| row.mjd < m);
        if index == 0 {
            return self.rows[0];
        }
        let (a, b) = (self.rows[index - 1], self.rows[index]);
        let w = (m - a.mjd) / (b.mjd - a.mjd);
        let da = dat(JulianDate::from(a.mjd + 2_400_000.5));
        let db = dat(JulianDate::from(b.mjd + 2_400_000.5));
        Eop {
            mjd: m,
            xp: a.xp + (b.xp - a.xp) * w,
            yp: a.yp + (b.yp - a.yp) * w,
            dx: a.dx + (b.dx - a.dx) * w,
            dy: a.dy + (b.dy - a.dy) * w,
            dut1: (a.dut1 - da) * (1.0 - w) + (b.dut1 - db) * w + dat(utc),
            predicted: a.predicted || b.predicted,
            available: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeContext {
    pub ut1: JulianDate,
    pub tt: JulianDate,
    pub tdb: JulianDate,
    pub delta_t_seconds: f64,
    pub eop: Eop,
    pub extrapolated: bool,
    pub note: String,
}
pub fn check_time_range(jd: JulianDate) -> Result<()> {
    let lo = to_jd(&CivilDate::new(-3000, 1, 1, 0, 0, 0.0), false)?;
    let hi = to_jd(&CivilDate::new(7000, 1, 1, 0, 0, 0.0), false)?;
    ensure!(
        jd.value().is_finite() && jd.value() >= lo.value() && jd.value() < hi.value(),
        "Date must be in astronomical years [-3000, 7000)"
    );
    Ok(())
}
pub fn make_time(
    mut input: JulianDate,
    scale: TimeScale,
    lon: f64,
    eops: &EarthOrientationData,
    override_dt: bool,
    manual: f64,
) -> Result<TimeContext> {
    ensure!(
        lon.is_finite()
            && manual.is_finite()
            && input.day.is_finite()
            && input.fraction.is_finite(),
        "Non-finite time/location input"
    );
    ensure!(decimal_year(input).is_finite(), "Invalid Julian date");
    let mut t = TimeContext::default();
    let input_tt = matches!(scale, TimeScale::TT | TimeScale::TDB);
    if scale == TimeScale::LocalMean {
        input = input.add_seconds(-lon * 240.0);
    }
    if scale == TimeScale::TDB {
        let d = unsafe { eraDtdb(input.day, input.fraction, 0.0, 0.0, 0.0, 0.0) };
        input = input.add_seconds(-d);
    }
    if scale == TimeScale::UTC {
        let limit = to_jd(&CivilDate::new(2027, 1, 1, 0, 0, 0.0), false)?;
        ensure!(
            decimal_year(input) >= 1972.0 && input.value() < limit.value(),
            "UTC needs EOP coverage and known leap seconds (before 2027; IERS C72); use UT1"
        );
        let (mut tai1, mut tai2) = (0.0, 0.0);
        ensure!(
            unsafe { eraUtctai(input.day, input.fraction, &mut tai1, &mut tai2) } >= 0,
            "Invalid UTC date"
        );
        t.tt = JulianDate {
            day: tai1,
            fraction: tai2,
        }
        .add_seconds(32.184);
        t.eop = eops.at(input);
        ensure!(t.eop.available, "No EOP coverage for UTC input; select UT1");
        let (mut u1, mut u2) = (0.0, 0.0);
        ensure!(
            unsafe { eraUtcut1(input.day, input.fraction, t.eop.dut1, &mut u1, &mut u2) } >= 0,
            "Invalid UTC date"
        );
        t.ut1 = JulianDate {
            day: u1,
            fraction: u2,
        };
        t.delta_t_seconds = ((t.tt.day - t.ut1.day) + (t.tt.fraction - t.ut1.fraction)) * 86400.0;
    } else {
        t.ut1 = input;
        for _ in 0..4 {
            let year = decimal_year(t.ut1);
            ensure!(
                year.is_finite(),
                "Invalid Julian date after time scale conversion"
            );
            let mut dt = if override_dt { manual } else { delta_t(year) };
            let mut utc = t.ut1;
            for _ in 0..2 {
                let eop = eops.at(utc);
                if eop.available {
                    utc = t.ut1.add_seconds(-eop.dut1);
                }
            }
            t.eop = eops.at(utc);
            if t.eop.available && !override_dt {
                dt = 32.184 + dat(utc) - t.eop.dut1;
            }
            t.delta_t_seconds = dt;
            if input_tt {
                t.ut1 = input.add_seconds(-dt);
            } else {
                break;
            }
        }
        t.tt = t.ut1.add_seconds(t.delta_t_seconds);
    }
    if scale == TimeScale::UTC && override_dt {
        t.delta_t_seconds = manual;
        t.tt = t.ut1.add_seconds(manual);
    }
    check_time_range(t.ut1)?;
    let y = decimal_year(t.tt);
    ensure!(y.is_finite(), "Invalid TT date after time scale conversion");
    t.tdb = t.tt.add_seconds(unsafe {
        eraDtdb(t.tt.day, t.tt.fraction, t.ut1.fraction, lon * RAD, 0.0, 0.0)
    });
    t.extrapolated = !(1900.0..=2100.0).contains(&y);
    t.note = if override_dt {
        "User ΔT"
    } else if t.eop.available {
        if t.eop.predicted {
            "EOP prediction"
        } else {
            "Observed EOP"
        }
    } else {
        "Model ΔT; polar motion unknown"
    }
    .to_string();
    if t.extrapolated {
        t.note
            .push_str("; long-term orientation / nutation extrapolation");
    }
    Ok(t)
}
fn utc_jd(c: &CivilDate) -> Result<JulianDate> {
    ensure!(c.second.is_finite(), "Invalid UTC date or leap second");
    let (mut day, mut fraction) = (0.0, 0.0);
    let status = unsafe {
        eraDtf2d(
            c"UTC".as_ptr(),
            c.year,
            c.month,
            c.day,
            c.hour,
            c.minute,
            c.second,
            &mut day,
            &mut fraction,
        )
    };
    ensure!((0..=1).contains(&status), "Invalid UTC date or leap second");
    Ok(JulianDate { day, fraction })
}
pub fn make_time_civil(
    c: &CivilDate,
    scale: TimeScale,
    lon: f64,
    eops: &EarthOrientationData,
    override_dt: bool,
    dt: f64,
    julian: bool,
) -> Result<TimeContext> {
    let input = if scale == TimeScale::UTC && !julian {
        utc_jd(c)?
    } else {
        to_jd(c, julian)?
    };
    make_time(input, scale, lon, eops, override_dt, dt)
}

/// Advance a clock by SI seconds, retaining UTC's leap second when applicable.
pub fn advance_date(
    c: &CivilDate,
    scale: TimeScale,
    seconds: f64,
    julian: bool,
) -> Result<CivilDate> {
    ensure!(seconds.is_finite(), "Non-finite time step");
    if scale != TimeScale::UTC || julian {
        return Ok(from_jd(to_jd(c, julian)?.add_seconds(seconds), julian));
    }
    let utc = utc_jd(c)?;
    let (mut a, mut b) = (0.0, 0.0);
    ensure!(
        unsafe { eraUtctai(utc.day, utc.fraction, &mut a, &mut b) } >= 0,
        "Invalid UTC date"
    );
    let tai = JulianDate {
        day: a,
        fraction: b,
    }
    .add_seconds(seconds);
    ensure!(
        unsafe { eraTaiutc(tai.day, tai.fraction, &mut a, &mut b) } >= 0,
        "Invalid UTC date"
    );
    let (mut year, mut month, mut day) = (0, 0, 0);
    let mut hms = [0; 4];
    let status = unsafe {
        eraD2dtf(
            c"UTC".as_ptr(),
            6,
            a,
            b,
            &mut year,
            &mut month,
            &mut day,
            hms.as_mut_ptr(),
        )
    };
    if status < 0 {
        bail!("Invalid UTC date");
    }
    Ok(CivilDate::new(
        year,
        month,
        day,
        hms[0],
        hms[1],
        hms[2] as f64 + hms[3] as f64 / 1e6,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn eop() -> EarthOrientationData {
        EarthOrientationData::new(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("data/time/finals2000A.all"),
        )
        .unwrap()
    }
    #[test]
    fn calendars_match_original_reference_cases() {
        assert!(
            (to_jd(&CivilDate::new(2000, 1, 1, 12, 0, 0.0), false)
                .unwrap()
                .value()
                - 2451545.0)
                .abs()
                < 1e-10
        );
        for year in [
            -3000, -2000, -100, -1, 0, 1, 4, 1582, 1900, 2000, 2026, 4000, 6999,
        ] {
            for julian in [false, true] {
                for month in 1..=12 {
                    let c = CivilDate::new(year, month, 17, 22, 15, 36.25);
                    let back = from_jd(to_jd(&c, julian).unwrap(), julian);
                    assert_eq!(
                        (c.year, c.month, c.day, c.hour, c.minute),
                        (back.year, back.month, back.day, back.hour, back.minute)
                    );
                    assert!((c.second - back.second).abs() < 1e-5);
                }
            }
        }
        assert!(valid_date(&CivilDate::new(0, 2, 29, 0, 0, 0.0), false));
        assert!(!valid_date(&CivilDate::new(1900, 2, 29, 0, 0, 0.0), false));
        assert!(valid_date(&CivilDate::new(1900, 2, 29, 0, 0, 0.0), true));
        assert!(to_jd(&CivilDate::new(2026, 2, 30, 0, 0, 0.0), false).is_err());
        assert!(
            check_time_range(to_jd(&CivilDate::new(7000, 1, 1, 0, 0, 0.0), false).unwrap())
                .is_err()
        );
        check_time_range(to_jd(&CivilDate::new(-3000, 1, 1, 0, 0, 0.0), false).unwrap()).unwrap();
        assert!((delta_t(2000.0) - 63.86).abs() < 1e-9);
        assert!((delta_t(-1000.0) - 25427.68).abs() < 1e-6);
    }
    #[test]
    fn utc_leaps_and_time_scales() {
        let eop = eop();
        let time = |c, scale, override_dt, dt| {
            make_time_civil(&c, scale, 0.0, &eop, override_dt, dt, false).unwrap()
        };
        let now = time(
            CivilDate::new(2026, 9, 4, 12, 0, 0.0),
            TimeScale::UTC,
            false,
            0.0,
        );
        assert!(now.eop.available);
        assert!((now.delta_t_seconds - (69.184 - now.eop.dut1)).abs() < 1e-5);
        let leap = time(
            CivilDate::new(2016, 12, 31, 23, 59, 60.0),
            TimeScale::UTC,
            false,
            0.0,
        );
        let next = time(
            CivilDate::new(2017, 1, 1, 0, 0, 0.0),
            TimeScale::UTC,
            false,
            0.0,
        );
        assert!(
            (((next.tt.day - leap.tt.day) + (next.tt.fraction - leap.tt.fraction)) * 86400.0 - 1.0)
                .abs()
                < 1e-8
        );
        assert!(
            make_time_civil(
                &CivilDate::new(2016, 12, 30, 23, 59, 60.0),
                TimeScale::UTC,
                0.0,
                &eop,
                false,
                0.0,
                false
            )
            .is_err()
        );
        assert!(
            make_time_civil(
                &CivilDate::new(6000, 1, 1, 0, 0, 0.0),
                TimeScale::UTC,
                0.0,
                &eop,
                false,
                0.0,
                false
            )
            .is_err()
        );
        let lmt = make_time_civil(
            &CivilDate::new(2000, 1, 2, 22, 0, 0.0),
            TimeScale::LocalMean,
            120.0,
            &eop,
            false,
            0.0,
            false,
        )
        .unwrap();
        assert!(
            (lmt.ut1.value()
                - to_jd(&CivilDate::new(2000, 1, 2, 14, 0, 0.0), false)
                    .unwrap()
                    .value())
            .abs()
                < 1e-9
        );
        let t1 = time(
            CivilDate::new(1000, 1, 1, 0, 0, 0.0),
            TimeScale::TT,
            true,
            100.0,
        );
        let t2 = time(
            CivilDate::new(1000, 1, 1, 0, 0, 0.0),
            TimeScale::TT,
            true,
            200.0,
        );
        assert!((t1.tt.value() - t2.tt.value()).abs() < 1e-9);
        assert!(((t1.ut1.value() - t2.ut1.value()) * 86400.0 - 100.0).abs() < 1e-4);
        let before = CivilDate::new(2016, 12, 31, 23, 59, 59.0);
        assert_eq!(
            advance_date(&before, TimeScale::UTC, 1.0, false).unwrap(),
            CivilDate::new(2016, 12, 31, 23, 59, 60.0)
        );
        assert_eq!(
            advance_date(&before, TimeScale::UTC, 2.0, false).unwrap(),
            CivilDate::new(2017, 1, 1, 0, 0, 0.0)
        );
    }
}
