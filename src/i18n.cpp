#include "astro/i18n.hpp"
#include "json.hpp"
#include <algorithm>
#include <array>
#include <cctype>
#include <cstring>
#include <fstream>
#include <stdexcept>

namespace astro {
namespace {
constexpr auto entries = std::to_array<Translation>({
    {"白昼", "Daylight"},
    {"民用晨昏", "Civil twilight"},
    {"航海晨昏", "Nautical twilight"},
    {"天文晨昏", "Astronomical twilight"},
    {"黑夜", "Night"},
    {"太阳中心高度 −6°", "Solar centre at −6°"},
    {"上次黎明", "Previous dawn"},
    {"下次黎明", "Next dawn"},
    {"上次黄昏", "Previous dusk"},
    {"下次黄昏", "Next dusk"},
    {"自动曝光", "Automatic exposure"},
    {"按全天亮度测光，转动和缩放不会改变曝光。关闭后使用固定夜空曝光。",
     "Meters the whole sky, independent of rotation and zoom. Turn off for fixed night-sky "
     "exposure."},
    {"大气预设", "Atmosphere preset"},
    {"清澈", "Clear"},
    {"薄霾", "Hazy"},
    {"370 天或数据有效期内没有找到对应的晨昏时刻。",
     "No matching twilight within 370 days or the supported date range."},
    {"万年星空", "A 10,000-year sky"},
    {"Astra · 万年星空", "Astra · Planetarium"},
    {"观察地点", "Observer"},
    {"搜索天体 / HIP 编号", "Find an object / HIP ID"},
    {"地点", "Site"},
    {"时间", "Time"},
    {"显示", "View"},
    {"选择一处地点，或输入地理坐标", "Choose a site or enter coordinates"},
    {"北京 Beijing", "Beijing"},
    {"上海 Shanghai", "Shanghai"},
    {"旧金山 San Francisco", "San Francisco"},
    {"阿塔卡马 Atacama", "Atacama"},
    {"赤道 Equator", "Equator"},
    {"北极 North Pole", "North Pole"},
    {"南极 South Pole", "South Pole"},
    {"自定义坐标", "Custom coordinates"},
    {"经度 / 东经为正", "Longitude / east positive"},
    {"纬度 / 北纬为正", "Latitude / north positive"},
    {"海拔 / 米", "Elevation / metres"},
    {"收藏地点", "Save site"},
    {"恢复收藏", "Restore site"},
    {"地点已收藏", "Site saved"},
    {"公元 2000 年 · 前后各 5000 年", "5,000 years either side of CE 2000"},
    {"天文年 / 含公元 0 年", "Astronomical year / includes year 0"},
    {"公元前 %d 年", "%d BCE"},
    {"月 / 日          时 / 分", "Month / day       Hour / minute"},
    {"时间标准", "Time standard"},
    {"UT1 · 地球自转时", "UT1 · Earth rotation"},
    {"TT · 地球时", "TT · Terrestrial time"},
    {"TDB · 历表时", "TDB · Dynamical time"},
    {"UTC · 现代民用时", "UTC · Modern civil time"},
    {"LMT · 当地平太阳时", "LMT · Local mean solar time"},
    {"使用儒略历", "Julian calendar"},
    {"前往这个时刻", "Go to time"},
    {"现在", "Now"},
    {"穿越一万年", "Explore 10,000 years"},
    {"%d 年", "%d"},
    {"该年代使用 UT1 地球自转时", "Using UT1 for this epoch"},
    {"大气与晨昏", "Atmosphere"},
    {"地面", "Ground"},
    {"天体名称", "Labels"},
    {"坐标网", "Grid"},
    {"银河光带", "Milky Way"},
    {"天空投影", "Projection"},
    {"直线透视", "Perspective"},
    {"球面广角（保角）", "Stereographic"},
    {"全天鱼眼", "Fisheye"},
    {"恢复水平  R", "Level horizon  R"},
    {"可见恒星 / 星等上限", "Stars / limiting magnitude"},
    {"曝光", "Exposure"},
    {"大气与时间模型###models", "Atmosphere & time###models"},
    {"气压 hPa", "Pressure / hPa"},
    {"温度 °C", "Temperature / °C"},
    {"光污染", "Light pollution"},
    {"自定义 ΔT", "Override ΔT"},
    {"ΔT 秒", "ΔT / seconds"},
    {"保存场景", "Save scene"},
    {"载入场景", "Load scene"},
    {"场景已保存", "Scene saved"},
    {"导出 PNG + 场景", "Export PNG + scene"},
    {"日期与时间", "Date & time"},
    {"天空与显示", "Sky & display"},
    {"月球", "MOON"},
    {"正在计算月球位置…", "Calculating the Moon…"},
    {"月面照明", "Illumination"},
    {"地平线以下", "Below the horizon"},
    {"已在地平线上方", "Above the horizon"},
    {"高度 %+6.1f°", "Alt %+6.1f°"},
    {"方位 %5.1f°", "Az %5.1f°"},
    {"距离 %s km", "Distance %s km"},
    {"拉近看月亮", "Zoom to Moon"},
    {"跳到可观月时刻", "Find a viewing time"},
    {"在未来 35 天内寻找：月球高度 ≥ 8°，太阳高度 ≤ −6°\n每半小时采样，保留当前地点和时间标准",
     "Search the next 35 days: Moon ≥ 8°, Sun ≤ −6°\nHalf-hour steps; keeps this site and time "
     "standard"},
    {"定位月球", "Focus Moon"},
    {"星空全景", "Panorama"},
    {"没有匹配的天体。可在显示设置中提高星等上限。",
     "No objects found. Try a higher magnitude limit in View settings."},
    {"跟踪月球", "Track Moon"},
    {"Esc 取消", "Esc to stop"},
    {"星等 %.2f", "Magnitude %.2f"},
    {"距离 %s %s", "Distance %s %s"},
    {"当前在地平线以下", "Currently below the horizon"},
    {"跟踪此天体", "Track object"},
    {"观测详情###details", "Observation details###details"},
    {"赤经 %.4f°", "RA %.4f°"},
    {"赤纬 %.4f°", "Dec %.4f°"},
    {"视方向 / ICRS 轴", "Apparent / ICRS axes"},
    {"形式误差约 %.2f″（不含模型）", "Formal error ~%.2f″\n(excludes model uncertainty)"},
    {"−1 日", "−1 d"},
    {"−1 时", "−1 h"},
    {"暂停", "Pause"},
    {"播放", "Play"},
    {"+1 时", "+1 h"},
    {"+1 日", "+1 d"},
    {"1 倍", "1×"},
    {"60 倍", "60×"},
    {"3600 倍", "3600×"},
    {"1 日 / 秒", "1 day/s"},
    {"1 年 / 秒", "1 year/s"},
    {"逆行", "Reverse"},
    {"顺行", "Forward"},
    {"拖动环顾 · 滚轮缩放 · R 恢复水平 · H 沉浸模式",
     "Drag to look · Scroll to zoom · R level horizon · H hide UI"},
    {"正在计算…  /  ", "Calculating…  /  "},
    {"北 N", "N"},
    {"东 E", "E"},
    {"南 S", "S"},
    {"西 W", "W"},
    {"正在读取星表与历表…", "Loading stars and ephemerides…"},
    {"数据未就绪", "Data not ready"},
    {"未来 35 天或数据有效期内没有合适的夜间观月时刻。可调整日期或地点。",
     "No suitable night-time Moon view within 35 days or the data interval. Try another date or "
     "site."},
    {"已前往可观月时刻：", "Viewing time: ", TranslationMode::Prefix},
    {"已前往晨昏时刻：", "Twilight time: ", TranslationMode::Prefix},
    {"截图已保存：", "Screenshot saved: ", TranslationMode::Prefix},
    {"关闭", "Close"},
    {"长期外推 · ΔT %.1f 秒 · 误差范围未知",
     "Long-term estimate · ΔT %.1f s · uncertainty unknown"},
    {"A S T R A   /   H 显示面板", "A S T R A   /   H show interface"},
    {"DE441 · 行星系统质心近似", "DE441 · planetary-system barycentre approximation"},
    {"DE441 · 站心视位置", "DE441 · topocentric apparent position"},
    {" · 径向速度未知", " · unknown radial velocity", TranslationMode::Fragment},
    {" · 距离未知", " · unknown distance", TranslationMode::Fragment},
    {" · 固定方向近似", " · fixed-direction approximation", TranslationMode::Fragment},
    {" · 多星/光心近似", " · multiple-star / photocentre approximation", TranslationMode::Fragment},
    {" · 低质量解", " · low-quality solution", TranslationMode::Fragment},
    {" · 运动传播近似警告", " · space-motion approximation warning", TranslationMode::Fragment},
    {" · HIP 位置匹配", " · HIP positional match", TranslationMode::Fragment},
    {"日期必须位于天文年 −3000 至 6999 之间", "Date must be in astronomical years [-3000, 7000)"},
    {"日期或时间无效", "Invalid calendar date or clock time"},
    {"该 UTC 日期不在已知闰秒范围内，请使用 UT1",
     "UTC needs EOP coverage and known leap seconds (before 2027; IERS C72); use UT1"},
    {"该 UTC 日期缺少地球方向数据，请使用 UT1", "No EOP coverage for UTC input; select UT1"},
    {"UTC 日期或闰秒无效", "Invalid UTC date or leap second"},
    {"时间或地点包含非有限值", "Non-finite time/location input"},
    {"地点、相机或大气参数无效", "Scenario contains invalid location, camera or atmosphere values"},
    {"无法写入场景", "Cannot write scenario"},
    {"无法打开场景，请先保存", "Cannot open scenario"},
    {"不支持的场景版本", "Unsupported scenario version"},
    {"场景星表版本与当前数据包不同", "Scenario data version differs from the installed data pack"},
    {"场景背景版本与当前地图不同", "Scenario background version differs from the installed map"},
    {"日期字段无效", "Invalid date tuple"},
    {"时间标准无效", "Invalid time scale"},
    {"未知天空投影", "Unknown sky projection"},
    {"语言设置保存失败：", "Could not save language preference: ", TranslationMode::Prefix},
    {"星表缺失或无效，请运行 scripts/build_catalog.py",
     "Missing/invalid star pack; run scripts/build_catalog.py"},
    {"星表文件不完整", "Truncated star pack"},
    {"缺少星表清单", "Star pack manifest missing"},
    {"太阳 Sun", "Sun"},
    {"月球 Moon", "Moon"},
    {"水星 Mercury", "Mercury"},
    {"金星 Venus", "Venus"},
    {"火星 Mars", "Mars"},
    {"木星 Jupiter", "Jupiter"},
    {"土星 Saturn", "Saturn"},
    {"天王星 Uranus", "Uranus"},
    {"海王星 Neptune", "Neptune"},
    {"北极星 Polaris", "Polaris"},
    {"水委一 Achernar", "Achernar"},
    {"毕宿五 Aldebaran", "Aldebaran"},
    {"五车二 Capella", "Capella"},
    {"参宿七 Rigel", "Rigel"},
    {"参宿四 Betelgeuse", "Betelgeuse"},
    {"老人星 Canopus", "Canopus"},
    {"天狼星 Sirius", "Sirius"},
    {"南河三 Procyon", "Procyon"},
    {"角宿一 Spica", "Spica"},
    {"马腹一 Hadar", "Hadar"},
    {"大角星 Arcturus", "Arcturus"},
    {"心宿二 Antares", "Antares"},
    {"织女星 Vega", "Vega"},
    {"牛郎星 Altair", "Altair"},
    {"天津四 Deneb", "Deneb"},
    {"北落师门 Fomalhaut", "Fomalhaut"},
    {"南门二 Alpha Centauri", "Alpha Centauri"},
    {"巴纳德星 Barnard's Star", "Barnard's Star"},
});
} // namespace

std::span<const Translation> translations() {
    return entries;
}

const char* Translator::operator()(const char* source) const {
    for (const auto& entry : entries) {
        if (std::strcmp(source, entry.chinese) == 0 || std::strcmp(source, entry.english) == 0) {
            return language == Language::Chinese ? entry.chinese : entry.english;
        }
    }
    return source;
}

std::string Translator::message(std::string_view source) const {
    std::string result(source);
    const char* exact = (*this)(result.c_str());
    if (exact != result.c_str()) {
        return exact;
    }
    for (const auto& entry : entries) {
        const std::string_view from = language == Language::English ? entry.chinese : entry.english;
        const std::string_view to = language == Language::English ? entry.english : entry.chinese;
        if (entry.mode == TranslationMode::Prefix && source.starts_with(from)) {
            return std::string(to) + std::string(source.substr(from.size()));
        }
        if (entry.mode == TranslationMode::Fragment) {
            if (const auto at = result.find(from); at != std::string::npos) {
                result.replace(at, from.size(), to);
            }
        }
    }
    return result;
}

std::optional<Language> parse_language(std::string_view locale) {
    auto base = std::string(locale.substr(0, locale.find_first_of("-_.@")));
    std::transform(base.begin(), base.end(), base.begin(), [](unsigned char ch) {
        return char(std::tolower(ch));
    });
    if (base == "en") {
        return Language::English;
    }
    if (base == "zh") {
        return Language::Chinese;
    }
    return std::nullopt;
}

const char* language_code(Language language) {
    return language == Language::Chinese ? "zh-CN" : "en";
}

Language load_language(const std::filesystem::path& path, Language fallback) {
    try {
        std::ifstream input(path);
        if (!input) {
            return fallback;
        }
        const auto json = nlohmann::json::parse(input);
        return parse_language(json.value("language", "")).value_or(fallback);
    } catch (const nlohmann::json::exception&) {
        return fallback;
    }
}

void save_language(const std::filesystem::path& path, Language language) {
    if (!path.parent_path().empty()) {
        std::filesystem::create_directories(path.parent_path());
    }
    std::ofstream output(path);
    output << nlohmann::json{{"language", language_code(language)}}.dump(2) << '\n';
    output.close();
    if (!output) {
        throw std::runtime_error("Could not save language preference: " + path.string());
    }
}
} // namespace astro
