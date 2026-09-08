#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <numbers>

namespace astro {
inline constexpr double pi = std::numbers::pi, rad = pi / 180., arcsec = rad / 3600.,
                        au_km = 149597870.7, c_kms = 299792.458;

struct Vec3 {
    double x{}, y{}, z{};

    double& operator[](int i) {
        return i == 0 ? x : i == 1 ? y : z;
    }

    double operator[](int i) const {
        return i == 0 ? x : i == 1 ? y : z;
    }

    Vec3 operator+(Vec3 b) const {
        return {x + b.x, y + b.y, z + b.z};
    }

    Vec3 operator-(Vec3 b) const {
        return {x - b.x, y - b.y, z - b.z};
    }

    Vec3 operator*(double s) const {
        return {x * s, y * s, z * s};
    }

    Vec3 operator/(double s) const {
        return *this * (1 / s);
    }
};

inline double dot(Vec3 a, Vec3 b) {
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

inline Vec3 cross(Vec3 a, Vec3 b) {
    return {a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x};
}

inline double norm(Vec3 a) {
    return std::sqrt(dot(a, a));
}

inline Vec3 unit(Vec3 a) {
    double n = norm(a);
    return n > 0 ? a / n : Vec3{0, 0, 1};
}

inline double angle(Vec3 a, Vec3 b) {
    return std::atan2(norm(cross(a, b)), dot(a, b));
}

inline Vec3 sphere(double ra, double de) {
    return {cos(de) * cos(ra), cos(de) * sin(ra), sin(de)};
}

inline double wrap(double a) {
    a = fmod(a, 2 * pi);
    return a < 0 ? a + 2 * pi : a;
}

struct Mat3 {
    double a[3][3]{{1, 0, 0}, {0, 1, 0}, {0, 0, 1}};

    Vec3 operator*(Vec3 v) const {
        return {dot({a[0][0], a[0][1], a[0][2]}, v),
                dot({a[1][0], a[1][1], a[1][2]}, v),
                dot({a[2][0], a[2][1], a[2][2]}, v)};
    }

    Mat3 transpose() const {
        Mat3 r;
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                r.a[i][j] = a[j][i];
            }
        }
        return r;
    }

    Mat3 operator*(const Mat3& b) const {
        Mat3 r;
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                r.a[i][j] = 0;
                for (int k = 0; k < 3; k++) {
                    r.a[i][j] += a[i][k] * b.a[k][j];
                }
            }
        }
        return r;
    }
};

inline Mat3 enu_basis(double lon, double lat) {
    Mat3 m;
    m.a[0][0] = -sin(lon);
    m.a[0][1] = cos(lon);
    m.a[0][2] = 0;
    m.a[1][0] = -sin(lat) * cos(lon);
    m.a[1][1] = -sin(lat) * sin(lon);
    m.a[1][2] = cos(lat);
    m.a[2][0] = cos(lat) * cos(lon);
    m.a[2][1] = cos(lat) * sin(lon);
    m.a[2][2] = sin(lat);
    return m;
}
} // namespace astro
