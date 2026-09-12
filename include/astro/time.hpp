#pragma once
#include <filesystem>
#include <string>
#include <vector>

namespace astro {
enum class TimeScale { UT1, TT, TDB, UTC, LocalMean };

struct JulianDate {
    double day = 2451544.5, fraction = .5;
    static JulianDate from(double jd);
    JulianDate add_seconds(double seconds) const;

    double value() const {
        return day + fraction;
    }

    double since_j2000() const {
        return (day - 2451545.) + fraction;
    }
};

struct CivilDate {
    int year = 2026, month = 9, day = 4, hour = 22, minute = 0;
    double second = 0;
    bool operator==(const CivilDate&) const = default;
};

bool valid_date(const CivilDate&, bool julian = false);
JulianDate to_jd(const CivilDate&, bool julian = false);
CivilDate from_jd(JulianDate, bool julian = false);
std::string format_date(const CivilDate&);
double delta_t(double year);
double decimal_year(JulianDate);
const char* scale_name(TimeScale);

struct Eop {
    double mjd{}, xp{}, yp{}, dut1{}, dx{}, dy{};
    bool available = false, predicted = false;
};

class EarthOrientationData {
    std::vector<Eop> rows_;

public:
    explicit EarthOrientationData(const std::filesystem::path& path);
    Eop at(JulianDate utc) const;

    double last_mjd() const {
        return rows_.empty() ? 0 : rows_.back().mjd;
    }
};

struct TimeContext {
    JulianDate ut1, tt, tdb;
    double delta_t_seconds{};
    Eop eop;
    bool extrapolated = false;
    std::string note;
};

TimeContext make_time(JulianDate input,
                      TimeScale scale,
                      double longitude,
                      const EarthOrientationData&,
                      bool override_dt = false,
                      double dt = 0);
TimeContext make_time(const CivilDate&,
                      TimeScale,
                      double longitude,
                      const EarthOrientationData&,
                      bool override_dt = false,
                      double dt = 0,
                      bool julian = false);
void check_time_range(JulianDate ut1);
} // namespace astro
