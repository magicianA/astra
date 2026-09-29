use anyhow::{Context, Result};
use serde_json::json;
use std::{borrow::Cow, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    Chinese,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationMode {
    Exact,
    Prefix,
    Fragment,
}
#[derive(Clone, Copy, Debug)]
pub struct Translation {
    pub chinese: &'static str,
    pub english: &'static str,
    pub mode: TranslationMode,
}

static ENTRIES: &[Translation] = &[
    Translation {
        chinese: "地平线需要 2 至 100000 组方位角与高度角",
        english: "A horizon needs 2 to 100000 azimuth/altitude samples",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线方位角应为 0 至 360 度，高度角为 −20 至 89 度",
        english: "Horizon azimuth must be 0..360 and altitude -20..89 degrees",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线首尾高度必须一致",
        english: "Horizon seam samples must agree",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线方位角重复",
        english: "Duplicate horizon azimuth",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法打开地平线文件",
        english: "Cannot open horizon file",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "CSV 每行应为方位角、高度角",
        english: "Expected azimuth,altitude in horizon CSV",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "搜索范围为 0 至 370 天，角距大于 0 且不超过 180 度",
        english: "Search interval must be 0..370 days and separation 0..180 degrees",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "至少选择一个太阳系天体",
        english: "Choose at least one solar-system body",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "请选择两个不同天体",
        english: "Choose two different targets",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "当前年代无法计算此目标",
        english: "Target unavailable for this epoch",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "对照年份无效",
        english: "Invalid comparison year",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线高度无效",
        english: "Invalid horizon altitude",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线采样数量无效",
        english: "Invalid horizon sample count",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "镜头路径的起点与终点必须使用相同投影",
        english: "Camera path needs matching projections",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出需要 2 至 100000 帧、每秒 1 至 120 帧，以及非零有效时间步长",
        english: "Invalid sequence: 2..100000 frames, 1..120 fps, finite nonzero time step",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法保存导出清单",
        english: "Cannot save sequence manifest",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "不支持的场景模型：",
        english: "Unsupported scene model: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "目标高度",
        english: "Target altitude",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "距月球",
        english: "Moon separation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "青色：目标  金色：太阳  紫色：月球；点击曲线前往",
        english: "Cyan: target · Gold: Sun · Violet: Moon. Click to visit.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "探索天空",
        english: "Explore the sky",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星座与年代",
        english: "Constellations & epochs",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "观测计划",
        english: "Observing planner",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地景",
        english: "Landscape",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "录制与导出",
        english: "Record & export",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天象搜索",
        english: "Event search",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星座连线",
        english: "Constellation lines",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星座名称",
        english: "Constellation names",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "定位星座",
        english: "Focus constellation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "连线跟随恒星运动；现代星座图形不代表古代星官。",
        english: "Lines follow stellar motion; modern figures do not represent ancient traditions.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "双年代并排对照",
        english: "Compare two epochs",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "对照年份",
        english: "Comparison year",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "两侧共用地点、月日、时刻和镜头。拖动同步环顾，在左侧选取天体。",
        english: "Both views share site, month, day, clock and camera. Drag either view; select objects on the left.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月面地形与阴影",
        english: "Lunar terrain & shadows",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地球照",
        english: "Earthshine",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月面姿态：DE440 数值天平动",
        english: "Lunar orientation: DE440 numerical libration",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月面姿态：IAU 近似，远年代未经精度验证",
        english: "Lunar orientation: IAU approximation; distant epochs are unvalidated",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "使用当前选中天体",
        english: "Use selected object",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "计算未来 24 小时",
        english: "Plan the next 24 hours",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "最高位置",
        english: "Highest altitude",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "升起",
        english: "Rise",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "落下",
        english: "Set",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "本区间没有升落事件。",
        english: "No rise or set in this interval.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "前往推荐时刻",
        english: "Visit recommended time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "本区间没有满足条件的观测窗口。",
        english: "No suitable observing window in this interval.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出观测表",
        english: "Export observing table",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "按高度、夜色、月光和地平线筛选；不包含天气预报。绿色窗口按 5 分钟采样。",
        english: "Uses altitude, darkness, moonlight and horizon; excludes weather. Green windows use 5-minute samples.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导入 CSV 的方位角、高度角两列，或 JSON points 数组。北方为 0°，向东增加。",
        english: "Import azimuth and altitude columns in CSV, or a JSON points array. North is 0°, increasing eastward.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "选择轮廓文件",
        english: "Choose horizon file",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "应用地平线",
        english: "Apply horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "当前地景",
        english: "Current landscape",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "平坦地平线",
        english: "Flat horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "恢复平坦地平线",
        english: "Reset horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "轮廓会遮挡天体，并参与升落和观测计划计算；保存场景时一并保存。",
        english: "The horizon masks objects and affects rise/set times and observing plans. It is saved with the scene.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出目录",
        english: "Export directory",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "选择导出位置",
        english: "Choose export location",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "帧数",
        english: "Frame count",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "每帧推进秒数",
        english: "Seconds per frame",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "视频帧率",
        english: "Video frame rate",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "同时导出 MP4",
        english: "Also export MP4",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星轨叠加图",
        english: "Star-trail composite",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "锁定曝光",
        english: "Lock exposure",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "镜头运动",
        english: "Camera movement",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "将当前视角设为终点",
        english: "Set current view as endpoint",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "开始时的视角作为起点；星轨使用逐帧变亮叠加，不等同于物理长曝光。",
        english: "Starts from the view at export time. Trails use a lighten composite, not physical long-exposure integration.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "开始导出",
        english: "Start export",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "正在导出",
        english: "Exporting",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "取消导出",
        english: "Cancel export",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "正在编码视频…",
        english: "Encoding video…",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "每帧等待精确计算，保存 PNG 和场景。MP4 使用 FFmpeg；取消后保留已完成帧。",
        english: "Waits for each exact time and saves PNG plus scene. MP4 uses FFmpeg. Cancelling keeps completed frames.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日食",
        english: "Solar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月食",
        english: "Lunar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天体接近",
        english: "Close approach",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "掩星",
        english: "Occultation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "第二目标使用选中天体",
        english: "Use selection as second target",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "搜索天数",
        english: "Days to search",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "最大角距（度）",
        english: "Maximum separation / degrees",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "搜索天象",
        english: "Search events",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "此区间未找到符合条件的天象。",
        english: "No matching events in this interval.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "最小角距",
        english: "Minimum separation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "开始接触",
        english: "First contact",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "结束接触",
        english: "Last contact",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "查看峰值",
        english: "View maximum",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "播放过程",
        english: "Play event",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "接触时刻采用球形天体模型；远年代受 ΔT 影响。月食红光及日食天空变暗为近似。",
        english: "Contacts use spherical bodies; distant epochs depend on ΔT. Lunar eclipse colour and solar eclipse sky darkening are approximate.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "后台计算中…",
        english: "Calculating in background…",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "取消计算",
        english: "Cancel calculation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "探索",
        english: "Explore",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "当地可见",
        english: "Locally visible",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "旧场景已迁移到当前模型，画面可能与原版本不同。",
        english: "Scene migrated to the current models; its appearance may differ from the original.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日全食",
        english: "Total solar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日环食",
        english: "Annular solar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日偏食",
        english: "Partial solar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月全食",
        english: "Total lunar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月偏食",
        english: "Partial lunar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "半影月食",
        english: "Penumbral lunar eclipse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "计算已取消",
        english: "Search cancelled",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "请先选中一个天体",
        english: "Select an object first",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法写入观测表",
        english: "Cannot write observation plan",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出目录必须为空",
        english: "Export directory must be empty",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出期间窗口大小改变",
        english: "Window size changed during export",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法启动 FFmpeg；PNG 已保存。请安装 FFmpeg 或设置 ASTRA_FFMPEG。",
        english: "Cannot start FFmpeg; PNG frames were saved. Install FFmpeg or set ASTRA_FFMPEG.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "FFmpeg 编码失败；PNG 已保存",
        english: "FFmpeg encoding failed; PNG frames were saved",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "没有匹配的天体。支持恒星名称、完整 HIP 或 Gaia DR3 编号。",
        english: "No objects found. Use a star name or a complete HIP or Gaia DR3 ID.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月面细节曝光",
        english: "Expose lunar detail",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天鹰座",
        english: "Aquila",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "仙女座",
        english: "Andromeda",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "玉夫座",
        english: "Sculptor",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天坛座",
        english: "Ara",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天秤座",
        english: "Libra",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "鲸鱼座",
        english: "Cetus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "白羊座",
        english: "Aries",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "盾牌座",
        english: "Scutum",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "罗盘座",
        english: "Pyxis",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "牧夫座",
        english: "Bootes",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "雕具座",
        english: "Caelum",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "蝘蜓座",
        english: "Chamaeleon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "巨蟹座",
        english: "Cancer",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "摩羯座",
        english: "Capricornus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "船底座",
        english: "Carina",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "仙后座",
        english: "Cassiopeia",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "半人马座",
        english: "Centaurus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "仙王座",
        english: "Cepheus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "后发座",
        english: "Coma Berenices",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "猎犬座",
        english: "Canes Venatici",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "御夫座",
        english: "Auriga",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天鸽座",
        english: "Columba",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "圆规座",
        english: "Circinus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "巨爵座",
        english: "Crater",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南冕座",
        english: "Corona Australis",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北冕座",
        english: "Corona Borealis",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "乌鸦座",
        english: "Corvus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南十字座",
        english: "Crux",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天鹅座",
        english: "Cygnus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "海豚座",
        english: "Delphinus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "剑鱼座",
        english: "Dorado",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天龙座",
        english: "Draco",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "矩尺座",
        english: "Norma",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "波江座",
        english: "Eridanus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天箭座",
        english: "Sagitta",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天炉座",
        english: "Fornax",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "双子座",
        english: "Gemini",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "鹿豹座",
        english: "Camelopardalis",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大犬座",
        english: "Canis Major",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大熊座",
        english: "Ursa Major",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天鹤座",
        english: "Grus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "武仙座",
        english: "Hercules",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "时钟座",
        english: "Horologium",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "长蛇座",
        english: "Hydra",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "水蛇座",
        english: "Hydrus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "印第安座",
        english: "Indus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "蝎虎座",
        english: "Lacerta",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "麒麟座",
        english: "Monoceros",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天兔座",
        english: "Lepus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "狮子座",
        english: "Leo",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "豺狼座",
        english: "Lupus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天猫座",
        english: "Lynx",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天琴座",
        english: "Lyra",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "唧筒座",
        english: "Antlia",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "显微镜座",
        english: "Microscopium",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "苍蝇座",
        english: "Musca",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南极座",
        english: "Octans",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天燕座",
        english: "Apus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "蛇夫座",
        english: "Ophiuchus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "猎户座",
        english: "Orion",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "孔雀座",
        english: "Pavo",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "飞马座",
        english: "Pegasus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "绘架座",
        english: "Pictor",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "英仙座",
        english: "Perseus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "小马座",
        english: "Equuleus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "小犬座",
        english: "Canis Minor",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "小狮座",
        english: "Leo Minor",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "狐狸座",
        english: "Vulpecula",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "小熊座",
        english: "Ursa Minor",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "凤凰座",
        english: "Phoenix",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "双鱼座",
        english: "Pisces",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南鱼座",
        english: "Piscis Austrinus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "飞鱼座",
        english: "Volans",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "船尾座",
        english: "Puppis",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "网罟座",
        english: "Reticulum",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "人马座",
        english: "Sagittarius",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天蝎座",
        english: "Scorpius",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "巨蛇座",
        english: "Serpens",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "六分仪座",
        english: "Sextans",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "山案座",
        english: "Mensa",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "金牛座",
        english: "Taurus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "望远镜座",
        english: "Telescopium",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "杜鹃座",
        english: "Tucana",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "三角座",
        english: "Triangulum",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南三角座",
        english: "Triangulum Australe",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "宝瓶座",
        english: "Aquarius",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "室女座",
        english: "Virgo",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "船帆座",
        english: "Vela",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出完成：",
        english: "Export complete: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "导出已取消，已完成帧保留在：",
        english: "Export cancelled; completed frames saved in: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "白昼",
        english: "Daylight",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "民用晨昏",
        english: "Civil twilight",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "航海晨昏",
        english: "Nautical twilight",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天文晨昏",
        english: "Astronomical twilight",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "黑夜",
        english: "Night",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "恢复昼夜变化",
        english: "Restore day & night",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大气已关闭，白天也会显示星空。",
        english: "Atmosphere is off: stars remain visible during the day.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "关闭后不渲染大气，白天也会显示星空。",
        english: "Disabling the atmosphere leaves stars visible even during daylight.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "启用大气与自动曝光，曝光补偿恢复为 0 EV。",
        english: "Enables atmosphere and automatic exposure, and resets compensation to 0 EV.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "太阳中心高度 −6°",
        english: "Solar centre at −6°",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "上次黎明",
        english: "Previous dawn",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "下次黎明",
        english: "Next dawn",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "上次黄昏",
        english: "Previous dusk",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "下次黄昏",
        english: "Next dusk",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "自动曝光",
        english: "Automatic exposure",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "自适应曝光",
        english: "Adaptive exposure",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "按当前画面测光，平滑适应明暗。启用时曝光补偿归零。",
        english: "Meters the current view and smoothly adapts to light and dark. Enabling resets exposure compensation to 0 EV.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "按全天亮度测光，转动和缩放不会改变曝光。关闭后使用固定夜空曝光。",
        english: "Meters the whole sky, independent of rotation and zoom. Turn off for fixed night-sky exposure.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大气预设",
        english: "Atmosphere preset",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "清澈",
        english: "Clear",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "薄霾",
        english: "Hazy",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "370 天或数据有效期内没有找到对应的晨昏时刻。",
        english: "No matching twilight within 370 days or the supported date range.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "万年星空",
        english: "A 10,000-year sky",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "Astra · 万年星空",
        english: "Astra · Planetarium",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "观察地点",
        english: "Observer",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "搜索天体 / HIP 编号",
        english: "Find an object / HIP ID",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地点",
        english: "Site",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "时间",
        english: "Time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "显示",
        english: "View",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "选择一处地点，或输入地理坐标",
        english: "Choose a site or enter coordinates",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北京 Beijing",
        english: "Beijing",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "上海 Shanghai",
        english: "Shanghai",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "旧金山 San Francisco",
        english: "San Francisco",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "阿塔卡马 Atacama",
        english: "Atacama",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "赤道 Equator",
        english: "Equator",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北极 North Pole",
        english: "North Pole",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南极 South Pole",
        english: "South Pole",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "自定义坐标",
        english: "Custom coordinates",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "经度 / 东经为正",
        english: "Longitude / east positive",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "纬度 / 北纬为正",
        english: "Latitude / north positive",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "海拔 / 米",
        english: "Elevation / metres",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "收藏地点",
        english: "Save site",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "恢复收藏",
        english: "Restore site",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地点已收藏",
        english: "Site saved",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "公元 2000 年 · 前后各 5000 年",
        english: "5,000 years either side of CE 2000",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天文年 / 含公元 0 年",
        english: "Astronomical year / includes year 0",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "公元前 %d 年",
        english: "%d BCE",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月 / 日          时 / 分",
        english: "Month / day       Hour / minute",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "时间标准",
        english: "Time standard",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "UT1 · 地球自转时",
        english: "UT1 · Earth rotation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "TT · 地球时",
        english: "TT · Terrestrial time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "TDB · 历表时",
        english: "TDB · Dynamical time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "UTC · 现代民用时",
        english: "UTC · Modern civil time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "LMT · 当地平太阳时",
        english: "LMT · Local mean solar time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "使用儒略历",
        english: "Julian calendar",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "前往这个时刻",
        english: "Go to time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "现在",
        english: "Now",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "穿越一万年",
        english: "Explore 10,000 years",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "%d 年",
        english: "%d",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "该年代使用 UT1 地球自转时",
        english: "Using UT1 for this epoch",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大气与晨昏",
        english: "Atmosphere",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地面",
        english: "Ground",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天体名称",
        english: "Labels",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "坐标网",
        english: "Grid",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "银河光带",
        english: "Milky Way",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天空投影",
        english: "Projection",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "直线透视",
        english: "Perspective",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "球面广角（保角）",
        english: "Stereographic",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "全天鱼眼",
        english: "Fisheye",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "恢复水平  R",
        english: "Level horizon  R",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "可见恒星 / 星等上限",
        english: "Stars / limiting magnitude",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "曝光",
        english: "Exposure",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大气与时间模型###models",
        english: "Atmosphere & time###models",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "气压 hPa",
        english: "Pressure / hPa",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "温度 °C",
        english: "Temperature / °C",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "光污染",
        english: "Light pollution",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "自定义 ΔT",
        english: "Override ΔT",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "ΔT 秒",
        english: "ΔT / seconds",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "保存场景",
        english: "Save scene",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "载入场景",
        english: "Load scene",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "场景已保存",
        english: "Scene saved",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "导出 PNG + 场景",
        english: "Export PNG + scene",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日期与时间",
        english: "Date & time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天空与显示",
        english: "Sky & display",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月球",
        english: "MOON",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "正在计算月球位置…",
        english: "Calculating the Moon…",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月面照明",
        english: "Illumination",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地平线以下",
        english: "Below the horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "已在地平线上方",
        english: "Above the horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "高度 %+6.1f°",
        english: "Alt %+6.1f°",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "方位 %5.1f°",
        english: "Az %5.1f°",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "距离 %s km",
        english: "Distance %s km",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "拉近看月亮",
        english: "Zoom to Moon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "跳到可观月时刻",
        english: "Find a viewing time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "在未来 35 天内寻找：月球高度 ≥ 8°，太阳高度 ≤ −6°\n每半小时采样，保留当前地点和时间标准",
        english: "Search the next 35 days: Moon ≥ 8°, Sun ≤ −6°\nHalf-hour steps; keeps this site and time standard",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "定位月球",
        english: "Focus Moon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星空全景",
        english: "Panorama",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "没有匹配的天体。可在显示设置中提高星等上限。",
        english: "No objects found. Try a higher magnitude limit in View settings.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "跟踪月球",
        english: "Track Moon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "Esc 取消",
        english: "Esc to stop",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星等 %.2f",
        english: "Magnitude %.2f",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "距离 %s %s",
        english: "Distance %s %s",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "当前在地平线以下",
        english: "Currently below the horizon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "跟踪此天体",
        english: "Track object",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "观测详情###details",
        english: "Observation details###details",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "赤经 %.4f°",
        english: "RA %.4f°",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "赤纬 %.4f°",
        english: "Dec %.4f°",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "视方向 / ICRS 轴",
        english: "Apparent / ICRS axes",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "形式误差约 %.2f″（不含模型）",
        english: "Formal error ~%.2f″\n(excludes model uncertainty)",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "−1 日",
        english: "−1 d",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "−1 时",
        english: "−1 h",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "暂停",
        english: "Pause",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "播放",
        english: "Play",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "+1 时",
        english: "+1 h",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "+1 日",
        english: "+1 d",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "1 倍",
        english: "1×",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "60 倍",
        english: "60×",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "3600 倍",
        english: "3600×",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "1 日 / 秒",
        english: "1 day/s",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "1 年 / 秒",
        english: "1 year/s",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "逆行",
        english: "Reverse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "顺行",
        english: "Forward",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "拖动环顾 · 滚轮缩放 · R 恢复水平 · H 沉浸模式",
        english: "Drag to look · Scroll to zoom · R level horizon · H hide UI",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "正在计算…  /  ",
        english: "Calculating…  /  ",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北 N",
        english: "N",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "东 E",
        english: "E",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南 S",
        english: "S",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "西 W",
        english: "W",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "正在读取星表与历表…",
        english: "Loading stars and ephemerides…",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "数据未就绪",
        english: "Data not ready",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "未来 35 天或数据有效期内没有合适的夜间观月时刻。可调整日期或地点。",
        english: "No suitable night-time Moon view within 35 days or the data interval. Try another date or site.",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "已前往可观月时刻：",
        english: "Viewing time: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "已前往晨昏时刻：",
        english: "Twilight time: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "截图已保存：",
        english: "Screenshot saved: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "关闭",
        english: "Close",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "长期外推 · ΔT %.1f 秒 · 误差范围未知",
        english: "Long-term estimate · ΔT %.1f s · uncertainty unknown",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "A S T R A   /   H 显示面板",
        english: "A S T R A   /   H show interface",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "DE441 · 行星系统质心近似",
        english: "DE441 · planetary-system barycentre approximation",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "DE441 · 站心视位置",
        english: "DE441 · topocentric apparent position",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: " · 径向速度未知",
        english: " · unknown radial velocity",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · 距离未知",
        english: " · unknown distance",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · 固定方向近似",
        english: " · fixed-direction approximation",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · 多星/光心近似",
        english: " · multiple-star / photocentre approximation",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · 低质量解",
        english: " · low-quality solution",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · 运动传播近似警告",
        english: " · space-motion approximation warning",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: " · HIP 位置匹配",
        english: " · HIP positional match",
        mode: TranslationMode::Fragment,
    },
    Translation {
        chinese: "日期必须位于天文年 −3000 至 6999 之间",
        english: "Date must be in astronomical years [-3000, 7000)",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日期或时间无效",
        english: "Invalid calendar date or clock time",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "该 UTC 日期不在已知闰秒范围内，请使用 UT1",
        english: "UTC needs EOP coverage and known leap seconds (before 2027; IERS C72); use UT1",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "该 UTC 日期缺少地球方向数据，请使用 UT1",
        english: "No EOP coverage for UTC input; select UT1",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "UTC 日期或闰秒无效",
        english: "Invalid UTC date or leap second",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "时间或地点包含非有限值",
        english: "Non-finite time/location input",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "地点、相机或大气参数无效",
        english: "Scenario contains invalid location, camera or atmosphere values",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法写入场景",
        english: "Cannot write scenario",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "无法打开场景，请先保存",
        english: "Cannot open scenario",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "不支持的场景版本",
        english: "Unsupported scenario version",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "场景星表版本与当前数据包不同",
        english: "Scenario data version differs from the installed data pack",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "场景背景版本与当前地图不同",
        english: "Scenario background version differs from the installed map",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "日期字段无效",
        english: "Invalid date tuple",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "时间标准无效",
        english: "Invalid time scale",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "未知天空投影",
        english: "Unknown sky projection",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "语言设置保存失败：",
        english: "Could not save language preference: ",
        mode: TranslationMode::Prefix,
    },
    Translation {
        chinese: "星表缺失或无效，请运行 scripts/build_catalog.py",
        english: "Missing/invalid star pack; run scripts/build_catalog.py",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "星表文件不完整",
        english: "Truncated star pack",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "缺少星表清单",
        english: "Star pack manifest missing",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "太阳 Sun",
        english: "Sun",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "月球 Moon",
        english: "Moon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "水星 Mercury",
        english: "Mercury",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "金星 Venus",
        english: "Venus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "火星 Mars",
        english: "Mars",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "木星 Jupiter",
        english: "Jupiter",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "土星 Saturn",
        english: "Saturn",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天王星 Uranus",
        english: "Uranus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "海王星 Neptune",
        english: "Neptune",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北极星 Polaris",
        english: "Polaris",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "水委一 Achernar",
        english: "Achernar",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "毕宿五 Aldebaran",
        english: "Aldebaran",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "五车二 Capella",
        english: "Capella",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "参宿七 Rigel",
        english: "Rigel",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "参宿四 Betelgeuse",
        english: "Betelgeuse",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "老人星 Canopus",
        english: "Canopus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天狼星 Sirius",
        english: "Sirius",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南河三 Procyon",
        english: "Procyon",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "角宿一 Spica",
        english: "Spica",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "马腹一 Hadar",
        english: "Hadar",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "大角星 Arcturus",
        english: "Arcturus",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "心宿二 Antares",
        english: "Antares",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "织女星 Vega",
        english: "Vega",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "牛郎星 Altair",
        english: "Altair",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "天津四 Deneb",
        english: "Deneb",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "北落师门 Fomalhaut",
        english: "Fomalhaut",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "南门二 Alpha Centauri",
        english: "Alpha Centauri",
        mode: TranslationMode::Exact,
    },
    Translation {
        chinese: "巴纳德星 Barnard's Star",
        english: "Barnard's Star",
        mode: TranslationMode::Exact,
    },
];

pub fn translations() -> &'static [Translation] {
    ENTRIES
}
pub fn parse_language(locale: &str) -> Option<Language> {
    match locale
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "en" => Some(Language::English),
        "zh" => Some(Language::Chinese),
        _ => None,
    }
}
pub fn language_code(language: Language) -> &'static str {
    match language {
        Language::Chinese => "zh-CN",
        Language::English => "en",
    }
}
pub fn load_language(path: impl AsRef<Path>, fallback: Language) -> Language {
    let value = std::fs::read(path)
        .ok()
        .and_then(|input| serde_json::from_slice::<serde_json::Value>(&input).ok());
    value
        .as_ref()
        .and_then(|v| v.get("language"))
        .and_then(serde_json::Value::as_str)
        .and_then(parse_language)
        .unwrap_or(fallback)
}
pub fn save_language(path: impl AsRef<Path>, language: Language) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({ "language": language_code(language) }))?
        ),
    )
    .with_context(|| format!("Could not save language preference: {}", path.display()))?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Translator {
    pub language: Language,
}
impl Translator {
    pub fn translate<'a>(&self, source: &'a str) -> &'a str {
        for entry in ENTRIES {
            if source == entry.chinese || source == entry.english {
                return match self.language {
                    Language::Chinese => entry.chinese,
                    Language::English => entry.english,
                };
            }
        }
        source
    }
    pub fn message<'a>(&self, source: &'a str) -> Cow<'a, str> {
        let exact = self.translate(source);
        if exact != source {
            return Cow::Borrowed(exact);
        }
        let mut result = Cow::Borrowed(source);
        for entry in ENTRIES {
            let (from, to) = match self.language {
                Language::English => (entry.chinese, entry.english),
                Language::Chinese => (entry.english, entry.chinese),
            };
            match entry.mode {
                TranslationMode::Prefix if source.starts_with(from) => {
                    return Cow::Owned(format!("{to}{}", &source[from.len()..]));
                }
                TranslationMode::Fragment => {
                    if let Some(at) = result.find(from) {
                        result.to_mut().replace_range(at..at + from.len(), to);
                    }
                }
                _ => {}
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn all_translations_bidirectional_unique_and_keep_widget_ids() {
        let en = Translator {
            language: Language::English,
        };
        let zh = Translator {
            language: Language::Chinese,
        };
        let mut chinese = HashSet::new();
        let mut english = HashSet::new();
        for entry in translations() {
            assert!(chinese.insert(entry.chinese));
            assert!(english.insert(entry.english));
            assert_eq!(en.translate(entry.chinese), entry.english);
            assert_eq!(zh.translate(entry.english), entry.chinese);
            if let Some((_, id)) = entry.chinese.split_once("###") {
                assert_eq!(entry.english.split_once("###").unwrap().1, id);
            }
        }
        assert_eq!(
            en.translate("My 自定义 observatory"),
            "My 自定义 observatory"
        );
        assert_eq!(
            en.message("截图已保存：/tmp/我的星空.png"),
            "Screenshot saved: /tmp/我的星空.png"
        );
        assert_eq!(
            en.message("Gaia DR3 · 径向速度未知 · 距离未知 · 多星/光心近似"),
            "Gaia DR3 · unknown radial velocity · unknown distance · multiple-star / photocentre approximation"
        );
    }
    #[test]
    fn locales_and_malformed_preference_fallback() {
        for locale in ["en", "en-US", "en_GB.UTF-8", "EN"] {
            assert_eq!(parse_language(locale), Some(Language::English));
        }
        for locale in ["zh", "zh-CN", "zh_Hans_CN", "ZH"] {
            assert_eq!(parse_language(locale), Some(Language::Chinese));
        }
        for locale in ["", "english", "fr-FR"] {
            assert_eq!(parse_language(locale), None);
        }
        let path =
            std::env::temp_dir().join(format!("astra-rust-i18n-{}.json", std::process::id()));
        save_language(&path, Language::Chinese).unwrap();
        assert_eq!(load_language(&path, Language::English), Language::Chinese);
        for input in [
            "{broken",
            "[]",
            "{\"language\":42}",
            "{\"language\":\"unknown\"}",
        ] {
            std::fs::write(&path, input).unwrap();
            assert_eq!(load_language(&path, Language::English), Language::English);
        }
        std::fs::remove_file(path).unwrap();
    }
}
