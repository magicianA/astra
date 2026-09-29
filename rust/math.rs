use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, Neg, Sub, SubAssign};

pub const PI: f64 = std::f64::consts::PI;
pub const RAD: f64 = PI / 180.0;
pub const ARCSEC: f64 = RAD / 3600.0;
pub const AU_KM: f64 = 149_597_870.7;
pub const C_KMS: f64 = 299_792.458;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}
impl From<[f64; 3]> for Vec3 {
    fn from(v: [f64; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
}
impl From<Vec3> for [f64; 3] {
    fn from(v: Vec3) -> Self {
        v.array()
    }
}
impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 index out of bounds"),
        }
    }
}
impl IndexMut<usize> for Vec3 {
    fn index_mut(&mut self, i: usize) -> &mut f64 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("Vec3 index out of bounds"),
        }
    }
}
impl Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, b: Self) {
        *self = *self + b;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, b: Self) {
        *self = *self - b;
    }
}
impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}
impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        self * (1.0 / s)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
pub fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}
pub fn unit(a: Vec3) -> Vec3 {
    let n = norm(a);
    if n > 0.0 {
        a / n
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    }
}
pub fn angle(a: Vec3, b: Vec3) -> f64 {
    norm(cross(a, b)).atan2(dot(a, b))
}
pub fn sphere(ra: f64, dec: f64) -> Vec3 {
    Vec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
}
pub fn wrap(a: f64) -> f64 {
    let a = a % (2.0 * PI);
    if a < 0.0 { a + 2.0 * PI } else { a }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mat3 {
    pub a: [[f64; 3]; 3],
}
impl Default for Mat3 {
    fn default() -> Self {
        Self {
            a: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }
}
impl Mat3 {
    pub fn transpose(self) -> Self {
        let mut result = Self::default();
        for i in 0..3 {
            for j in 0..3 {
                result.a[i][j] = self.a[j][i];
            }
        }
        result
    }
}
impl Mul<Vec3> for Mat3 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        Vec3::new(
            dot(self.a[0].into(), v),
            dot(self.a[1].into(), v),
            dot(self.a[2].into(), v),
        )
    }
}
impl Mul for Mat3 {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        let mut result = Self::default();
        for i in 0..3 {
            for j in 0..3 {
                result.a[i][j] = (0..3).map(|k| self.a[i][k] * b.a[k][j]).sum();
            }
        }
        result
    }
}
pub fn enu_basis(lon: f64, lat: f64) -> Mat3 {
    Mat3 {
        a: [
            [-lon.sin(), lon.cos(), 0.0],
            [-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos()],
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bases_are_orthogonal_and_preserve_vectors() {
        for lat in [-PI / 2.0, 0.0, 39.9 * RAD, PI / 2.0] {
            for lon in [-70.0 * RAD, 0.0, 116.4 * RAD] {
                let basis = enu_basis(lon, lat);
                let identity = basis * basis.transpose();
                for i in 0..3 {
                    for j in 0..3 {
                        assert!((identity.a[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-14);
                    }
                }
                let v = sphere(0.41, -0.23);
                assert!(angle(v, basis.transpose() * (basis * v)) < 1e-14);
            }
        }
        assert_eq!(unit(Vec3::default()), Vec3::new(0.0, 0.0, 1.0));
    }
}
