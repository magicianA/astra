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
    {"地平线需要 2 至 100000 组方位角与高度角",
     "A horizon needs 2 to 100000 azimuth/altitude samples"},
    {"地平线方位角应为 0 至 360 度，高度角为 −20 至 89 度",
     "Horizon azimuth must be 0..360 and altitude -20..89 degrees"},
    {"地平线首尾高度必须一致", "Horizon seam samples must agree"},
    {"地平线方位角重复", "Duplicate horizon azimuth"},
    {"无法打开地平线文件", "Cannot open horizon file"},
    {"CSV 每行应为方位角、高度角", "Expected azimuth,altitude in horizon CSV"},
    {"搜索范围为 0 至 370 天，角距大于 0 且不超过 180 度",
     "Search interval must be 0..370 days and separation 0..180 degrees"},
    {"至少选择一个太阳系天体", "Choose at least one solar-system body"},
    {"请选择两个不同天体", "Choose two different targets"},
    {"当前年代无法计算此目标", "Target unavailable for this epoch"},
    {"对照年份无效", "Invalid comparison year"},
    {"地平线高度无效", "Invalid horizon altitude"},
    {"地平线采样数量无效", "Invalid horizon sample count"},
    {"镜头路径的起点与终点必须使用相同投影", "Camera path needs matching projections"},
    {"导出需要 2 至 100000 帧、每秒 1 至 120 帧，以及非零有效时间步长",
     "Invalid sequence: 2..100000 frames, 1..120 fps, finite nonzero time step"},
    {"无法保存导出清单", "Cannot save sequence manifest"},
    {"不支持的场景模型：", "Unsupported scene model: ", TranslationMode::Prefix},
    {"目标高度", "Target altitude"},
    {"距月球", "Moon separation"},
    {"青色：目标  金色：太阳  紫色：月球；点击曲线前往",
     "Cyan: target · Gold: Sun · Violet: Moon. Click to visit."},
    {"探索天空", "Explore the sky"},
    {"星座与年代", "Constellations & epochs"},
    {"观测计划", "Observing planner"},
    {"地景", "Landscape"},
    {"录制与导出", "Record & export"},
    {"天象搜索", "Event search"},
    {"星座连线", "Constellation lines"},
    {"星座名称", "Constellation names"},
    {"定位星座", "Focus constellation"},
    {"连线跟随恒星运动；现代星座图形不代表古代星官。",
     "Lines follow stellar motion; modern figures do not represent ancient traditions."},
    {"双年代并排对照", "Compare two epochs"},
    {"对照年份", "Comparison year"},
    {"两侧共用地点、月日、时刻和镜头。拖动同步环顾，在左侧选取天体。",
     "Both views share site, month, day, clock and camera. Drag either view; select objects on the "
     "left."},
    {"月面地形与阴影", "Lunar terrain & shadows"},
    {"地球照", "Earthshine"},
    {"月面姿态：DE440 数值天平动", "Lunar orientation: DE440 numerical libration"},
    {"月面姿态：IAU 近似，远年代未经精度验证",
     "Lunar orientation: IAU approximation; distant epochs are unvalidated"},
    {"使用当前选中天体", "Use selected object"},
    {"计算未来 24 小时", "Plan the next 24 hours"},
    {"最高位置", "Highest altitude"},
    {"升起", "Rise"},
    {"落下", "Set"},
    {"本区间没有升落事件。", "No rise or set in this interval."},
    {"前往推荐时刻", "Visit recommended time"},
    {"本区间没有满足条件的观测窗口。", "No suitable observing window in this interval."},
    {"导出观测表", "Export observing table"},
    {"按高度、夜色、月光和地平线筛选；不包含天气预报。绿色窗口按 5 分钟采样。",
     "Uses altitude, darkness, moonlight and horizon; excludes weather. Green windows use 5-minute "
     "samples."},
    {"导入 CSV 的方位角、高度角两列，或 JSON points 数组。北方为 0°，向东增加。",
     "Import azimuth and altitude columns in CSV, or a JSON points array. North is 0°, increasing "
     "eastward."},
    {"选择轮廓文件", "Choose horizon file"},
    {"应用地平线", "Apply horizon"},
    {"当前地景", "Current landscape"},
    {"平坦地平线", "Flat horizon"},
    {"恢复平坦地平线", "Reset horizon"},
    {"轮廓会遮挡天体，并参与升落和观测计划计算；保存场景时一并保存。",
     "The horizon masks objects and affects rise/set times and observing plans. It is saved with "
     "the scene."},
    {"导出目录", "Export directory"},
    {"选择导出位置", "Choose export location"},
    {"帧数", "Frame count"},
    {"每帧推进秒数", "Seconds per frame"},
    {"视频帧率", "Video frame rate"},
    {"同时导出 MP4", "Also export MP4"},
    {"星轨叠加图", "Star-trail composite"},
    {"锁定曝光", "Lock exposure"},
    {"镜头运动", "Camera movement"},
    {"将当前视角设为终点", "Set current view as endpoint"},
    {"开始时的视角作为起点；星轨使用逐帧变亮叠加，不等同于物理长曝光。",
     "Starts from the view at export time. Trails use a lighten composite, not physical "
     "long-exposure integration."},
    {"开始导出", "Start export"},
    {"正在导出", "Exporting"},
    {"取消导出", "Cancel export"},
    {"正在编码视频…", "Encoding video…"},
    {"每帧等待精确计算，保存 PNG 和场景。MP4 使用 FFmpeg；取消后保留已完成帧。",
     "Waits for each exact time and saves PNG plus scene. MP4 uses FFmpeg. Cancelling keeps "
     "completed frames."},
    {"日食", "Solar eclipse"},
    {"月食", "Lunar eclipse"},
    {"天体接近", "Close approach"},
    {"掩星", "Occultation"},
    {"第二目标使用选中天体", "Use selection as second target"},
    {"搜索天数", "Days to search"},
    {"最大角距（度）", "Maximum separation / degrees"},
    {"搜索天象", "Search events"},
    {"此区间未找到符合条件的天象。", "No matching events in this interval."},
    {"最小角距", "Minimum separation"},
    {"开始接触", "First contact"},
    {"结束接触", "Last contact"},
    {"查看峰值", "View maximum"},
    {"播放过程", "Play event"},
    {"接触时刻采用球形天体模型；远年代受 ΔT 影响。月食红光及日食天空变暗为近似。",
     "Contacts use spherical bodies; distant epochs depend on ΔT. Lunar eclipse colour and solar "
     "eclipse sky darkening are approximate."},
    {"后台计算中…", "Calculating in background…"},
    {"取消计算", "Cancel calculation"},
    {"探索", "Explore"},
    {"当地可见", "Locally visible"},
    {"旧场景已迁移到当前模型，画面可能与原版本不同。",
     "Scene migrated to the current models; its appearance may differ from the original."},
    {"日全食", "Total solar eclipse"},
    {"日环食", "Annular solar eclipse"},
    {"日偏食", "Partial solar eclipse"},
    {"月全食", "Total lunar eclipse"},
    {"月偏食", "Partial lunar eclipse"},
    {"半影月食", "Penumbral lunar eclipse"},
    {"计算已取消", "Search cancelled"},
    {"请先选中一个天体", "Select an object first"},
    {"无法写入观测表", "Cannot write observation plan"},
    {"导出目录必须为空", "Export directory must be empty"},
    {"导出期间窗口大小改变", "Window size changed during export"},
    {"无法启动 FFmpeg；PNG 已保存。请安装 FFmpeg 或设置 ASTRA_FFMPEG。",
     "Cannot start FFmpeg; PNG frames were saved. Install FFmpeg or set ASTRA_FFMPEG."},
    {"FFmpeg 编码失败；PNG 已保存", "FFmpeg encoding failed; PNG frames were saved"},
    {"没有匹配的天体。支持恒星名称、完整 HIP 或 Gaia DR3 编号。",
     "No objects found. Use a star name or a complete HIP or Gaia DR3 ID."},
    {"月面细节曝光", "Expose lunar detail"},
    {"天鹰座", "Aquila"},
    {"仙女座", "Andromeda"},
    {"玉夫座", "Sculptor"},
    {"天坛座", "Ara"},
    {"天秤座", "Libra"},
    {"鲸鱼座", "Cetus"},
    {"白羊座", "Aries"},
    {"盾牌座", "Scutum"},
    {"罗盘座", "Pyxis"},
    {"牧夫座", "Bootes"},
    {"雕具座", "Caelum"},
    {"蝘蜓座", "Chamaeleon"},
    {"巨蟹座", "Cancer"},
    {"摩羯座", "Capricornus"},
    {"船底座", "Carina"},
    {"仙后座", "Cassiopeia"},
    {"半人马座", "Centaurus"},
    {"仙王座", "Cepheus"},
    {"后发座", "Coma Berenices"},
    {"猎犬座", "Canes Venatici"},
    {"御夫座", "Auriga"},
    {"天鸽座", "Columba"},
    {"圆规座", "Circinus"},
    {"巨爵座", "Crater"},
    {"南冕座", "Corona Australis"},
    {"北冕座", "Corona Borealis"},
    {"乌鸦座", "Corvus"},
    {"南十字座", "Crux"},
    {"天鹅座", "Cygnus"},
    {"海豚座", "Delphinus"},
    {"剑鱼座", "Dorado"},
    {"天龙座", "Draco"},
    {"矩尺座", "Norma"},
    {"波江座", "Eridanus"},
    {"天箭座", "Sagitta"},
    {"天炉座", "Fornax"},
    {"双子座", "Gemini"},
    {"鹿豹座", "Camelopardalis"},
    {"大犬座", "Canis Major"},
    {"大熊座", "Ursa Major"},
    {"天鹤座", "Grus"},
    {"武仙座", "Hercules"},
    {"时钟座", "Horologium"},
    {"长蛇座", "Hydra"},
    {"水蛇座", "Hydrus"},
    {"印第安座", "Indus"},
    {"蝎虎座", "Lacerta"},
    {"麒麟座", "Monoceros"},
    {"天兔座", "Lepus"},
    {"狮子座", "Leo"},
    {"豺狼座", "Lupus"},
    {"天猫座", "Lynx"},
    {"天琴座", "Lyra"},
    {"唧筒座", "Antlia"},
    {"显微镜座", "Microscopium"},
    {"苍蝇座", "Musca"},
    {"南极座", "Octans"},
    {"天燕座", "Apus"},
    {"蛇夫座", "Ophiuchus"},
    {"猎户座", "Orion"},
    {"孔雀座", "Pavo"},
    {"飞马座", "Pegasus"},
    {"绘架座", "Pictor"},
    {"英仙座", "Perseus"},
    {"小马座", "Equuleus"},
    {"小犬座", "Canis Minor"},
    {"小狮座", "Leo Minor"},
    {"狐狸座", "Vulpecula"},
    {"小熊座", "Ursa Minor"},
    {"凤凰座", "Phoenix"},
    {"双鱼座", "Pisces"},
    {"南鱼座", "Piscis Austrinus"},
    {"飞鱼座", "Volans"},
    {"船尾座", "Puppis"},
    {"网罟座", "Reticulum"},
    {"人马座", "Sagittarius"},
    {"天蝎座", "Scorpius"},
    {"巨蛇座", "Serpens"},
    {"六分仪座", "Sextans"},
    {"山案座", "Mensa"},
    {"金牛座", "Taurus"},
    {"望远镜座", "Telescopium"},
    {"杜鹃座", "Tucana"},
    {"三角座", "Triangulum"},
    {"南三角座", "Triangulum Australe"},
    {"宝瓶座", "Aquarius"},
    {"室女座", "Virgo"},
    {"船帆座", "Vela"},
    {"导出完成：", "Export complete: ", TranslationMode::Prefix},
    {"导出已取消，已完成帧保留在：",
     "Export cancelled; completed frames saved in: ",
     TranslationMode::Prefix},
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
    {"自适应曝光", "Adaptive exposure"},
    {"按当前画面测光，平滑适应明暗。启用时曝光补偿归零。",
     "Meters the current view and smoothly adapts to light and dark. Enabling resets exposure "
     "compensation to 0 EV."},
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
