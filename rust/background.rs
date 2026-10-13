use crate::math::Mat3;

pub const BACKGROUND_ID: &str = "nasa-gaia-dr2-photometric-16k-v2";

/// Unit quaternion (x, y, z, w), mapping the local sky into Galactic axes.
pub fn galactic_rotation(celestial_to_enu: &Mat3) -> [f32; 4] {
    // ICRS-to-Galactic rotation used by ERFA/SOFA, expressed directly so no
    // coordinate conversion is necessary while building each render frame.
    let icrs_to_galactic = Mat3 {
        a: [
            [
                -0.0548755604162154,
                -0.8734370902348850,
                -0.4838350155487132,
            ],
            [0.4941094278755837, -0.4448296299600112, 0.7469822444972189],
            [-0.8676661490190047, -0.1980763734312015, 0.4559837761750669],
        ],
    };
    let m = (icrs_to_galactic * celestial_to_enu.transpose()).a;
    let mut q = [0.0; 4];
    let trace = m[0][0] + m[1][1] + m[2][2];
    if trace > 0.0 {
        let scale = 2.0 * (1.0 + trace).sqrt();
        q = [
            (m[2][1] - m[1][2]) / scale,
            (m[0][2] - m[2][0]) / scale,
            (m[1][0] - m[0][1]) / scale,
            scale / 4.0,
        ];
    } else {
        let mut i = 0;
        if m[1][1] > m[i][i] {
            i = 1;
        }
        if m[2][2] > m[i][i] {
            i = 2;
        }
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        let scale = 2.0 * (1.0 + m[i][i] - m[j][j] - m[k][k]).sqrt();
        q[i] = scale / 4.0;
        q[j] = (m[j][i] + m[i][j]) / scale;
        q[k] = (m[k][i] + m[i][k]) / scale;
        q[3] = (m[k][j] - m[j][k]) / scale;
    }
    let length = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    q.map(|v| (v / length) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn galactic_rotation_is_unit_and_maps_icrs_pole() {
        let q = galactic_rotation(&Mat3::default());
        assert!((q.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() - 1.0).abs() < 1e-7);
        let [x, y, z, w] = q.map(f64::from);
        // The north Galactic pole is transformed into the positive z axis.
        let v = [-0.8676661490190047, -0.1980763734312015, 0.4559837761750669];
        let rz = 2.0 * (x * z - w * y) * v[0]
            + 2.0 * (y * z + w * x) * v[1]
            + (1.0 - 2.0 * (x * x + y * y)) * v[2];
        assert!((rz - 1.0).abs() < 1e-7);
    }
}
