#include "astro/camera.hpp"

namespace astro {
double zoom_fov(double fov, ProjectionKind kind, double steps) {
    return std::clamp(fov * exp(-steps * .12), rad, maximum_zoom_fov(kind));
}

Vec3 Camera::forward() const {
    return {sin(azimuth) * cos(elevation), cos(azimuth) * cos(elevation), sin(elevation)};
}

Vec3 Camera::right() const {
    const Vec3 horizontal{cos(azimuth), -sin(azimuth), 0};
    return horizontal * cos(roll) + cross(horizontal, forward()) * sin(roll);
}

Vec3 Camera::up() const {
    return cross(right(), forward());
}

double Camera::pixels_per_radian() const {
    const double field = std::min(fov, maximum_fov(projection));
    switch (projection) {
    case ProjectionKind::Fisheye:
        return std::min(width, height) / field;
    case ProjectionKind::Stereographic:
        return height / (4 * tan(field / 4));
    default:
        return height / (2 * tan(field / 2));
    }
}

Projection Camera::prepare() const {
    return {right(),
            up(),
            forward(),
            pixels_per_radian(),
            width,
            height,
            std::min(fov, maximum_fov(projection)) / 2,
            projection};
}

ScreenPoint Camera::project(Vec3 v) const {
    return prepare().project(v);
}

ScreenPoint Projection::project(Vec3 v) const {
    double x = dot(v, right), y = dot(v, up), z = dot(v, forward);
    if (kind == ProjectionKind::Perspective) {
        if (z <= 0) {
            return {};
        }
        x = x / z * scale;
        y = y / z * scale;
    } else if (kind == ProjectionKind::Stereographic) {
        // Stereographic projection is conformal: small sky features retain
        // their shape at the edge instead of stretching into radial streaks.
        const double denominator = norm(v) + z;
        if (denominator <= 1e-12) {
            return {};
        }
        x *= 2 * scale / denominator;
        y *= 2 * scale / denominator;
    } else {
        double theta = atan2(hypot(x, y), z), r = hypot(x, y);
        if (theta > half_fov) {
            return {};
        }
        if (r > 1e-15) {
            x = x / r * theta * scale;
            y = y / r * theta * scale;
        } else {
            x = y = 0;
        }
    }
    x += width / 2;
    y = height / 2 - y;
    return {x, y, x >= -30 && y >= -30 && x <= width + 30 && y <= height + 30};
}

Vec3 Camera::unproject(double x, double y) const {
    x = (x - width / 2) / pixels_per_radian();
    y = (height / 2 - y) / pixels_per_radian();
    if (projection == ProjectionKind::Perspective) {
        return unit(forward() + right() * x + up() * y);
    }
    if (projection == ProjectionKind::Stereographic) {
        const double r2 = (x * x + y * y) / 4;
        return (forward() * (1 - r2) + right() * x + up() * y) / (1 + r2);
    }
    double r = hypot(x, y);
    return r < 1e-15 ? forward() : forward() * cos(r) + (right() * x + up() * y) * (sin(r) / r);
}

bool Camera::contains(double x, double y) const {
    return x >= 0 && y >= 0 && x < width && y < height &&
           (projection != ProjectionKind::Fisheye ||
            hypot(x - width / 2, y - height / 2) <= std::min(width, height) / 2);
}

void Camera::pan(double from_x, double from_y, double to_x, double to_y) {
    const double scale = pixels_per_radian();
    const auto angle_at = [&](double offset) {
        switch (projection) {
        case ProjectionKind::Fisheye:
            return offset;
        case ProjectionKind::Stereographic:
            return 2 * atan(offset / 2);
        default:
            return atan(offset);
        }
    };
    const auto horizontal_angle = [&](double x) {
        return angle_at((x - width / 2) / scale);
    };
    const auto vertical_angle = [&](double y) {
        return angle_at((height / 2 - y) / scale);
    };
    // Turn left/right and look up/down about the local horizon. A drag must
    // never add camera roll: that makes the entire sky spin like a flat card.
    const double yaw = horizontal_angle(from_x) - horizontal_angle(to_x);
    const double pitch = vertical_angle(from_y) - vertical_angle(to_y);
    azimuth = wrap(azimuth + yaw);
    elevation = std::clamp(elevation + pitch, -pi / 2, pi / 2);
}

double Projection::corner_angle() const {
    const double radius = hypot(width + 60, height + 60) / (2 * scale);
    switch (kind) {
    case ProjectionKind::Fisheye:
        return half_fov;
    case ProjectionKind::Stereographic:
        return 2 * atan(radius / 2);
    default:
        return atan(radius);
    }
}

double Projection::maximum_scale(ScreenPoint point) const {
    const double radius = hypot(point.x - width / 2, point.y - height / 2) / scale;
    switch (kind) {
    case ProjectionKind::Fisheye:
        return radius < 1e-12 ? scale : scale * radius / sin(radius);
    case ProjectionKind::Stereographic:
        return scale * (1 + radius * radius / 4);
    default:
        return scale * (1 + radius * radius);
    }
}

} // namespace astro
