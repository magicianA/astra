#include "astro/time.hpp"
#include "astro/math.hpp"
#include <algorithm>
#include <erfa.h>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <stdexcept>

namespace astro {
JulianDate JulianDate::from(double jd) {
    double d = floor(jd - .5) + .5;
    return {d, jd - d};
}

JulianDate JulianDate::add_seconds(double s) const {
    double f = fraction + s / 86400.;
    double whole = floor(f);
    return {day + whole, f - whole};
}

static bool leap(int y, bool julian) {
    return y % 4 == 0 && (julian || y % 100 != 0 || y % 400 == 0);
}

bool valid_date(const CivilDate& c, bool julian) {
    static int days[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    return c.year >= -4799 && c.year <= 9999 && c.month >= 1 && c.month <= 12 && c.day >= 1 &&
           c.day <= days[c.month - 1] + (c.month == 2 && leap(c.year, julian)) && c.hour >= 0 &&
           c.hour < 24 && c.minute >= 0 && c.minute < 60 && std::isfinite(c.second) &&
           c.second >= 0 && c.second < 60;
}

JulianDate to_jd(const CivilDate& c, bool julian) {
    if (!valid_date(c, julian)) {
        throw std::invalid_argument("Invalid calendar date or clock time");
    }
    int y = c.year, m = c.month;
    if (m <= 2) {
        --y;
        m += 12;
    }
    double b = julian ? 0 : 2 - floor(y / 100.) + floor(y / 400.);
    return {floor(365.25 * (y + 4716)) + floor(30.6001 * (m + 1)) + c.day + b - 1524.5,
            (c.hour * 3600. + c.minute * 60. + c.second) / 86400.};
}

CivilDate from_jd(JulianDate jd, bool julian) {
    double z = floor(jd.day + jd.fraction + .5);
    double f = (jd.day - (z - .5)) + jd.fraction;
    double a = z;
    if (!julian) {
        double alpha = floor((z - 1867216.25) / 36524.25);
        a = z + 1 + alpha - floor(alpha / 4);
    }
    double b = a + 1524, c = floor((b - 122.1) / 365.25), d = floor(365.25 * c),
           e = floor((b - d) / 30.6001);
    CivilDate r;
    r.day = int(b - d - floor(30.6001 * e));
    r.month = int(e < 14 ? e - 1 : e - 13);
    r.year = int(r.month > 2 ? c - 4716 : c - 4715);
    double secs = std::max(0., f * 86400.);
    r.hour = int(secs / 3600);
    secs -= r.hour * 3600;
    r.minute = int(secs / 60);
    r.second = secs - r.minute * 60;
    return r;
}

std::string format_date(const CivilDate& c) {
    std::ostringstream o;
    if (c.year <= 0) {
        o << "BCE " << 1 - c.year;
    } else {
        o << "CE " << c.year;
    }
    o << '-' << std::setfill('0') << std::setw(2) << c.month << '-' << std::setw(2) << c.day << "  "
      << std::setw(2) << c.hour << ':' << std::setw(2) << c.minute << ':' << std::setw(2)
      << int(c.second);
    return o.str();
}

double decimal_year(JulianDate jd) {
    auto c = from_jd(jd);
    auto start = to_jd({c.year, 1, 1, 0, 0, 0});
    return c.year + ((jd.day - start.day) + jd.fraction) / (leap(c.year, false) ? 366. : 365.);
}

// Espenak & Meeus, NASA Five Millennium Canon. Outside its historic
// constraints this is explicitly a nominal extrapolation, not a prediction.
double delta_t(double y) {
    double t, u;
    if (y < -500 || y >= 2150) {
        u = (y - 1820) / 100;
        return -20 + 32 * u * u;
    }
    if (y < 500) {
        u = y / 100;
        return 10583.6 - 1014.41 * u + 33.78311 * pow(u, 2) - 5.952053 * pow(u, 3) -
               .1798452 * pow(u, 4) + .022174192 * pow(u, 5) + .0090316521 * pow(u, 6);
    }
    if (y < 1600) {
        u = (y - 1000) / 100;
        return 1574.2 - 556.01 * u + 71.23472 * u * u + .319781 * pow(u, 3) - .8503463 * pow(u, 4) -
               .005050998 * pow(u, 5) + .0083572073 * pow(u, 6);
    }
    if (y < 1700) {
        t = y - 1600;
        return 120 - .9808 * t - .01532 * t * t + pow(t, 3) / 7129;
    }
    if (y < 1800) {
        t = y - 1700;
        return 8.83 + .1603 * t - .0059285 * t * t + .00013336 * pow(t, 3) - pow(t, 4) / 1174000;
    }
    if (y < 1860) {
        t = y - 1800;
        return 13.72 - .332447 * t + .0068612 * t * t + .0041116 * pow(t, 3) -
               .00037436 * pow(t, 4) + .0000121272 * pow(t, 5) - .0000001699 * pow(t, 6) +
               .000000000875 * pow(t, 7);
    }
    if (y < 1900) {
        t = y - 1860;
        return 7.62 + .5737 * t - .251754 * t * t + .01680668 * pow(t, 3) -
               .0004473624 * pow(t, 4) + pow(t, 5) / 233174;
    }
    if (y < 1920) {
        t = y - 1900;
        return -2.79 + 1.494119 * t - .0598939 * t * t + .0061966 * pow(t, 3) - .000197 * pow(t, 4);
    }
    if (y < 1941) {
        t = y - 1920;
        return 21.20 + .84493 * t - .076100 * t * t + .0020936 * pow(t, 3);
    }
    if (y < 1961) {
        t = y - 1950;
        return 29.07 + .407 * t - t * t / 233 + pow(t, 3) / 2547;
    }
    if (y < 1986) {
        t = y - 1975;
        return 45.45 + 1.067 * t - t * t / 260 - pow(t, 3) / 718;
    }
    if (y < 2005) {
        t = y - 2000;
        return 63.86 + .3345 * t - .060374 * t * t + .0017275 * pow(t, 3) + .000651814 * pow(t, 4) +
               .00002373599 * pow(t, 5);
    }
    if (y < 2050) {
        t = y - 2000;
        return 62.92 + .32217 * t + .005589 * t * t;
    }
    u = (y - 1820) / 100;
    return -20 + 32 * u * u - .5628 * (2150 - y);
}

const char* scale_name(TimeScale s) {
    switch (s) {
    case TimeScale::UT1:
        return "UT1";
    case TimeScale::TT:
        return "TT";
    case TimeScale::TDB:
        return "TDB";
    case TimeScale::UTC:
        return "UTC";
    case TimeScale::LocalMean:
        return "LMT";
    }
    return "?";
}

static double field(const std::string& s, size_t pos, size_t count) {
    return std::stod(s.substr(pos, count));
}

static double dat(JulianDate jd) {
    auto c = from_jd(jd);
    double result = 0;
    eraDat(c.year, c.month, c.day, jd.fraction, &result);
    return result;
}

EarthOrientationData::EarthOrientationData(const std::filesystem::path& p) {
    std::ifstream in(p);
    std::string s;
    while (std::getline(in, s)) {
        try {
            if (s.size() < 125) {
                continue;
            }
            Eop e;
            e.mjd = field(s, 7, 8);
            e.xp = field(s, 18, 9) * arcsec;
            e.yp = field(s, 37, 9) * arcsec;
            e.dut1 = field(s, 58, 10);
            e.available = true;
            e.predicted = s[16] == 'P' || s[57] == 'P';
            try {
                e.dx = field(s, 97, 9) * arcsec / 1000.;
                e.dy = field(s, 116, 9) * arcsec / 1000.;
            } catch (...) {
            }
            rows_.push_back(e);
        } catch (...) {
        }
    }
}

Eop EarthOrientationData::at(JulianDate utc) const {
    double m = (utc.day - 2400000.5) + utc.fraction;
    if (rows_.empty() || m < rows_.front().mjd || m > rows_.back().mjd) {
        return {};
    }
    auto it = std::lower_bound(rows_.begin(), rows_.end(), m, [](const Eop& e, double v) {
        return e.mjd < v;
    });
    if (it == rows_.begin()) {
        return *it;
    }
    auto a = *(it - 1), b = *it;
    double w = (m - a.mjd) / (b.mjd - a.mjd);
    Eop r = a;
    r.mjd = m;
    r.xp += (b.xp - a.xp) * w;
    r.yp += (b.yp - a.yp) * w;
    r.dx += (b.dx - a.dx) * w;
    r.dy += (b.dy - a.dy) * w;
    double da = dat(JulianDate::from(a.mjd + 2400000.5)),
           db = dat(JulianDate::from(b.mjd + 2400000.5));
    r.dut1 = (a.dut1 - da) * (1 - w) + (b.dut1 - db) * w + dat(utc);
    r.predicted = a.predicted || b.predicted;
    return r;
}

void check_time_range(JulianDate jd) {
    static auto lo = to_jd({-3000, 1, 1, 0, 0, 0}), hi = to_jd({7000, 1, 1, 0, 0, 0});
    if (!std::isfinite(jd.value()) || jd.value() < lo.value() || jd.value() >= hi.value()) {
        throw std::out_of_range("Date must be in astronomical years [-3000, 7000)");
    }
}

TimeContext make_time(JulianDate input,
                      TimeScale scale,
                      double lon,
                      const EarthOrientationData& eops,
                      bool over,
                      double manual) {
    if (!std::isfinite(lon) || !std::isfinite(manual)) {
        throw std::invalid_argument("Non-finite time/location input");
    }
    TimeContext t;
    bool input_tt = scale == TimeScale::TT || scale == TimeScale::TDB;
    if (scale == TimeScale::LocalMean) {
        input = input.add_seconds(-lon * 240.);
    }
    if (scale == TimeScale::TDB) {
        double d = eraDtdb(input.day, input.fraction, 0, 0, 0, 0);
        input = input.add_seconds(-d);
    }
    if (scale == TimeScale::UTC) {
        // IERS Bulletin C 72 confirms no leap second at the end of 2026.
        if (decimal_year(input) < 1972 || input.value() >= to_jd({2027, 1, 1, 0, 0, 0}).value()) {
            throw std::invalid_argument(
                "UTC needs EOP coverage and known leap seconds (before 2027; IERS C72); use UT1");
        }
        double tai1, tai2;
        eraUtctai(input.day, input.fraction, &tai1, &tai2);
        t.tt = JulianDate{tai1, tai2}.add_seconds(32.184);
        t.eop = eops.at(input);
        if (!t.eop.available) {
            throw std::runtime_error("No EOP coverage for UTC input; select UT1");
        }
        double u1, u2;
        eraUtcut1(input.day, input.fraction, t.eop.dut1, &u1, &u2);
        t.ut1 = {u1, u2};
        t.delta_t_seconds = ((t.tt.day - t.ut1.day) + (t.tt.fraction - t.ut1.fraction)) * 86400.;
    } else {
        t.ut1 = input;
        for (int i = 0; i < 4; i++) {
            double dt = over ? manual : delta_t(decimal_year(t.ut1));
            auto utc = t.ut1;
            for (int j = 0; j < 2; j++) {
                auto e = eops.at(utc);
                if (e.available) {
                    utc = t.ut1.add_seconds(-e.dut1);
                }
            }
            t.eop = eops.at(utc);
            if (t.eop.available && !over) {
                dt = 32.184 + dat(utc) - t.eop.dut1;
            }
            t.delta_t_seconds = dt;
            if (input_tt) {
                t.ut1 = input.add_seconds(-dt);
            } else {
                break;
            }
        }
        t.tt = t.ut1.add_seconds(t.delta_t_seconds);
    }
    if (scale == TimeScale::UTC && over) {
        t.delta_t_seconds = manual;
        t.tt = t.ut1.add_seconds(manual);
    }
    check_time_range(t.ut1);
    t.tdb = t.tt.add_seconds(eraDtdb(t.tt.day, t.tt.fraction, t.ut1.fraction, lon * rad, 0, 0));
    double y = decimal_year(t.tt);
    t.extrapolated = y < 1900 || y > 2100;
    t.note = over              ? "User ΔT"
             : t.eop.available ? (t.eop.predicted ? "EOP prediction" : "Observed EOP")
                               : "Model ΔT; polar motion unknown";
    if (t.extrapolated) {
        t.note += "; long-term orientation / nutation extrapolation";
    }
    return t;
}

TimeContext make_time(const CivilDate& c,
                      TimeScale s,
                      double lon,
                      const EarthOrientationData& e,
                      bool over,
                      double dt,
                      bool julian) {
    if (s == TimeScale::UTC && !julian) {
        double a, b;
        int status = eraDtf2d("UTC", c.year, c.month, c.day, c.hour, c.minute, c.second, &a, &b);
        if (status < 0 || status > 1) {
            throw std::invalid_argument("Invalid UTC date or leap second");
        }
        return make_time(JulianDate{a, b}, s, lon, e, over, dt);
    }
    return make_time(to_jd(c, julian), s, lon, e, over, dt);
}
} // namespace astro
