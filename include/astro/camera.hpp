#pragma once
#include "math.hpp"

namespace astro {
// Values also identify the projection in the Vulkan scene push constants.
enum class ProjectionKind { Perspective = 0, Fisheye = 1, Stereographic = 2 };

inline constexpr double default_camera_fov = 60 * rad;

constexpr double maximum_fov(ProjectionKind kind) {
    return (kind == ProjectionKind::Perspective ? 150 : 180) * rad;
}

// Interactive perspective zoom stops at the normal panorama. Wider fields are
// still supported when explicitly loaded from a saved scene.
constexpr double maximum_zoom_fov(ProjectionKind kind) {
    return kind == ProjectionKind::Perspective ? default_camera_fov : maximum_fov(kind);
}

// FOV is in radians; positive wheel steps zoom in.
double zoom_fov(double fov, ProjectionKind kind, double steps);

struct Projection;

struct ScreenPoint {
    double x{}, y{};
    bool visible = false;
};

struct Camera {
    double azimuth = pi, elevation = 35 * rad, roll = 0, fov = default_camera_fov, width = 1280,
           height = 800;
    ProjectionKind projection = ProjectionKind::Perspective;
    Projection prepare() const;
    Vec3 forward() const;
    Vec3 right() const;
    Vec3 up() const;
    ScreenPoint project(Vec3 direction) const;
    Vec3 unproject(double x, double y) const;
    double pixels_per_radian() const;
    bool contains(double x, double y) const;
    // Pan in azimuth/elevation without introducing roll. Coordinates are logical pixels.
    void pan(double from_x, double from_y, double to_x, double to_y);
};

// Camera basis and scale are constant while projecting a frame's catalogue.
struct Projection {
    Vec3 right, up, forward;
    double scale, width, height, half_fov;
    ProjectionKind kind;
    ScreenPoint project(Vec3 direction) const;
    // Cone enclosing the viewport and the point-sprite culling margin.
    double corner_angle() const;
    // Largest local angular scale, for resolved body sizes and their labels.
    double maximum_scale(ScreenPoint point) const;
};
} // namespace astro
