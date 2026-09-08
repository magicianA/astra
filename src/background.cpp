#include "astro/background.hpp"
#include <erfa.h>

namespace astro {
std::array<float, 4> galactic_rotation(const Mat3& celestial_to_enu) {
    static const Mat3 icrs_to_galactic = [] {
        Mat3 matrix;
        for (int i = 0; i < 3; ++i) {
            double ra, dec;
            eraG2icrs(i == 1 ? pi / 2 : 0, i == 2 ? pi / 2 : 0, &ra, &dec);
            const auto axis = sphere(ra, dec);
            for (int j = 0; j < 3; ++j) {
                matrix.a[i][j] = axis[j];
            }
        }
        return matrix;
    }();
    const auto matrix = icrs_to_galactic * celestial_to_enu.transpose();
    const auto& m = matrix.a;
    std::array<double, 4> q{};
    const double trace = m[0][0] + m[1][1] + m[2][2];
    if (trace > 0) {
        const double scale = 2 * sqrt(1 + trace);
        q = {(m[2][1] - m[1][2]) / scale,
             (m[0][2] - m[2][0]) / scale,
             (m[1][0] - m[0][1]) / scale,
             scale / 4};
    } else {
        int i = 0;
        if (m[1][1] > m[i][i]) {
            i = 1;
        }
        if (m[2][2] > m[i][i]) {
            i = 2;
        }
        const int j = (i + 1) % 3, k = (i + 2) % 3;
        const double scale = 2 * sqrt(1 + m[i][i] - m[j][j] - m[k][k]);
        q[i] = scale / 4;
        q[j] = (m[j][i] + m[i][j]) / scale;
        q[k] = (m[k][i] + m[i][k]) / scale;
        q[3] = (m[k][j] - m[j][k]) / scale;
    }
    const double length = sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]);
    return {float(q[0] / length), float(q[1] / length), float(q[2] / length), float(q[3] / length)};
}
} // namespace astro
