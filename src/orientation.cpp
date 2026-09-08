#include "astro/orientation.hpp"
#include <cstring>
#include <erfa.h>
#include <fstream>
#include <stdexcept>

namespace astro {
static double modern_weight(double y) {
    if (y >= 1900 && y <= 2100) {
        return 1.;
    }
    double x = y < 1900 ? (y - 1850) / 50. : (2150 - y) / 50.;
    x = std::clamp(x, 0., 1.);
    return x * x * (3 - 2 * x);
}

Vec3 Orientation::pole(double days) {
    double y = 2000 + days / 365.25;
    Mat3 p;
    eraLtpb(y, p.a);
    double eq[3], ec[3];
    eraLtpequ(y, eq);
    eraLtpecl(y, ec);
    double eps = acos(std::clamp(eq[0] * ec[0] + eq[1] * ec[1] + eq[2] * ec[2], -1., 1.));
    double dp, de;
    eraNut00a(2451545., days, &dp, &de);
    Mat3 n;
    eraNumat(eps, dp, de, n.a);
    auto m = n * p;
    Vec3 v{m.a[2][0], m.a[2][1], m.a[2][2]};
    double w = modern_weight(y);
    if (w > 0) {
        double x, z, s;
        eraXys06a(2451545., days, &x, &z, &s);
        Vec3 modern{x, z, sqrt(std::max(0., 1 - x * x - z * z))};
        v = unit(v * (1 - w) + modern * w);
    }
    return v;
}

static double area(Vec3 a, Vec3 b) {
    return 2 * atan2(cross(a, b).z, 1 + a.z + b.z + dot(a, b));
}

void Orientation::generate(const std::filesystem::path& path, double step) {
    if (step <= 0 || step > 32) {
        throw std::invalid_argument("CIO step must be (0,32] days");
    }
    int each = int(ceil(5002. * 365.25 / step));
    std::vector<double> values(size_t(each) * 2 + 1);
    double x, y, s;
    eraXys06a(2451545, 0, &x, &y, &s);
    values[each] = s;
    Vec3 zero = pole(0), a = zero;
    for (int i = 1; i <= each; i++) {
        auto b = pole(i * step);
        values[each + i] = values[each + i - 1] - area(a, b);
        a = b;
    }
    a = zero;
    for (int i = 1; i <= each; i++) {
        auto b = pole(-i * step);
        values[each - i] = values[each - i + 1] - area(a, b);
        a = b;
    }
    std::filesystem::create_directories(path.parent_path());
    std::ofstream out(path.string() + ".partial", std::ios::binary);
    out.write("ACIO0001", 8);
    double start = -each * step;
    uint64_t count = values.size();
    out.write((char*)&start, 8);
    out.write((char*)&step, 8);
    out.write((char*)&count, 8);
    out.write((char*)values.data(), std::streamsize(values.size() * 8));
    out.close();
    if (!out) {
        throw std::runtime_error("Writing CIO table failed");
    }
    std::filesystem::rename(path.string() + ".partial", path);
}

Orientation::Orientation(const std::filesystem::path& p) {
    std::ifstream in(p, std::ios::binary);
    char magic[8];
    uint64_t count = 0;
    in.read(magic, 8);
    in.read((char*)&start_, 8);
    in.read((char*)&step_, 8);
    in.read((char*)&count, 8);
    if (!in || memcmp(magic, "ACIO0001", 8) || step_ <= 0 || count > 10000000 || count < 2) {
        throw std::runtime_error(
            "Missing or invalid CIO table; run astra_cli --generate-cio DATA/time/cio.bin");
    }
    s_.resize(count);
    in.read((char*)s_.data(), count * 8);
    if (!in) {
        throw std::runtime_error("Truncated CIO table");
    }
}

double Orientation::cio(double days) const {
    double index = (days - start_) / step_;
    if (index < 0 || index >= double(s_.size() - 1)) {
        throw std::out_of_range("CIO date outside cache");
    }
    size_t i = size_t(index);
    double w = index - i;
    return s_[i] * (1 - w) + s_[i + 1] * w;
}

Mat3 Orientation::celestial(const TimeContext& t) const {
    double days = t.tt.since_j2000(), year = 2000 + days / 365.25;
    double x, y, s;
    double w = modern_weight(year);
    if (w == 1) {
        eraXys06a(t.tt.day, t.tt.fraction, &x, &y, &s);
    } else {
        auto p = pole(days);
        x = p.x;
        y = p.y;
        s = cio(days);
        if (w > 0) {
            double mx, my, ms;
            eraXys06a(t.tt.day, t.tt.fraction, &mx, &my, &ms);
            s = s * (1 - w) + ms * w;
        }
    }
    if (t.eop.available) {
        x += t.eop.dx;
        y += t.eop.dy;
    }
    Mat3 result;
    eraC2ixys(x, y, s, result.a);
    return result;
}

Mat3 Orientation::terrestrial(const TimeContext& t) const {
    Mat3 result = celestial(t), pm;
    eraRz(eraEra00(t.ut1.day, t.ut1.fraction), result.a);
    eraPom00(t.eop.xp, t.eop.yp, t.extrapolated ? 0 : eraSp00(t.tt.day, t.tt.fraction), pm.a);
    return pm * result;
}
} // namespace astro
