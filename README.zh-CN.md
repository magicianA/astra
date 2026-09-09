# Astra · 万年星空

[English](README.md) · 简体中文

原生 C++20 / SDL3 / Vulkan 桌面星空模拟器。macOS 通过 MoltenVK 使用 Metal GPU。
输入地球经纬度、海拔、日期和时刻，查看恒星、太阳、月球及七颗其他行星的站心视位置。

已加入光谱大气散射产生的黎明与黄昏，包括瑞利散射、米氏多次散射和臭氧吸收。
“时间”面板可跳转上次／下次民用黎明或黄昏，“显示”面板可选择自动曝光与清澈／薄霾预设。
自动曝光不随拖动、缩放改变。示例见 `examples/dawn.json`、`examples/dusk.json`；
[实现、限制与实测结果](docs/TWILIGHT_IMPLEMENTATION.md)记录了模型细节。

界面支持**简体中文和英文**。在“显示 / View”面板顶部的“Language / 语言”中即时切换，
首次启动跟随系统首选的受支持语言，其他情况回退到英文。手动选择保存在用户目录的
`ui-preferences.json`，重启后继续使用；切换保留时刻、地点、视角、选中天体和播放状态。
两种界面都能搜索中英文天体名称。启动参数 `--language en` 或 `--language zh-CN` 可临时
指定语言；只有在界面中修改语言时才覆盖已保存的偏好。

时间范围为天文年 **−3000-01-01 至 +7000-01-01，不含右端点**，默认使用延伸格里高利历。
天文年 0 是公元前 1 年，−3000 是公元前 3001 年。远古和未来使用 UT1、TT、TDB 或当地平太阳时；
UTC 只在闰秒和地球方向数据可用的现代时段开放。

## 直接运行本机版本

```sh
open dist/Astra.app
```

应用包带运行库、字体、星表和 DE441，无需额外设置 `VULKAN_SDK` 或联网。
这是为本机制作的 arm64 应用；已在 Apple M4 Pro / macOS 26.4.1 上测试。
本机包所用 ERFA 库要求 macOS 26，其他 macOS 版本需使用对应部署目标重新编译依赖。
Windows、Linux 构建路径保留，尚未测试。

拖动天空调整方向，滚轮缩放，点击天体查看资料。左侧工具栏展开地点、时间或显示设置；
万年时间轴位于时间面板，场景保存与 PNG 导出位于显示面板。顶部可搜索天体，底部可逐时、
逐日跳转或正反向播放。按 Space 播放/暂停，方向键转向，R 恢复水平，H 进入沉浸模式，Esc 关闭面板并取消跟踪。
拖动按真实窗口尺寸与视场换算为转头、抬头，保持地平线水平，不再自动倾斜整片天空。
仰角到天顶或天底时停止；旧场景若带有倾斜，可用 R 或显示面板中的“恢复水平”归正。
默认采用直线透视，地平线保持平直；垂直视场为 60°，避免默认超广角造成明显的边缘拉伸。
普通透视的滚轮缩放范围为 1°–60°，缩回全景后继续滚动也不会进入超广角；从其他投影切回透视时同样限制到 60°。
“星空全景”也恢复到这一设置。显示面板可手动选择球面广角或全天鱼眼，它们会弯曲离开
画面中心的线条。直线透视和球面广角的 FOV 表示垂直视场；切换投影保留当前朝向。
旧版保存的场景保持原投影，可在显示面板切换后重新保存。地面可关闭，便于查看地平线以下的天空。

右侧月球卡片持续显示月相、照明比例、高度和距离。默认时刻的月球在地平线下约 3.4°；
点击“跳到可观月时刻”，会在后台每半小时采样未来 35 天，寻找月球高度至少 8°、太阳高度
低于 −6°、月面照明至少 2% 的时刻，并定位放大到月球。搜索保留地点和时间标准，不修改地平线遮挡；
这是观赏时刻建议，不是精确月出时刻。极昼等没有结果的情况会提示，搜索也受数据有效期限制。
“定位月球”保留当前时间，“拉近看月亮”调整视角，“星空全景”恢复广角。

默认地点是北京，时间为 2026-09-04 22:00 当地平太阳时。它与法定时区时间有区别；现代民用输入
可选择 UTC。“现在”按钮填入当前 UTC。场景、收藏和手动导出保存在
`~/Library/Application Support/Astra/Sky/`。

## 数据和计算

- Gaia DR3 的全天 `G ≤ 12` 查询，与 Hipparcos-2 合并；BSC5 补充亮星名称、颜色和部分径向速度。
  原始目录与完整 ADQL 保留，按 source_id 去重并核对独立计数。准确规模见 `data/catalog/manifest.json`。
- JPL DE441 两个完整 SPK 分段，约 3 GB，覆盖整个目标时间段。太阳、地球、月球、水星和金星使用
  相应天体中心；火星至海王星使用行星系统质心，并在信息面板注明。
- CPU 双精度天文计算：恒星空间运动、视差、光行时、太阳引力偏折、光行差、地球自转、岁差、
  章动、极移、站心坐标和可选近地平折射。使用 ERFA 与 CSPICE。
- 1900–2100 年使用现代 IAU 姿态模型，远期使用 Vondrák 长期岁差和数值 CIO 表，过渡段平滑连接。
  未知地球自转采用标明为估计的 ΔT 模型，也可手动调整。
- Vulkan 双缓冲、HDR 天空与星点、太阳/月球/行星圆面与相位、曝光映射、中文界面和截图。
  恒星空间运动与视差等较重计算在工作线程进行。播放时逐帧计算地球姿态和太阳系视位置，
  用当前姿态投影缓存的恒星视方向；暂停后补齐停止时刻的完整计算，截图使用该精确快照。

**数据覆盖不等于万年内具有同一精度。** ΔT、远期章动、未知恒星径向速度、双星光心和恒星演化都
限制远期可靠性。当前误差栏是对角形式误差估计，未完成完整协方差传播。G/Hp/V 光度并未统一为
严格的 Johnson V，显示亮度和颜色属于观感近似。星表的 `G ≤ 12` 是目录历元的筛选，不能保证远期
所有可能变亮的更暗恒星都被收录。大气模型不包含真实天气，地平面不包含地形。

## 从源码构建（macOS）

需要 Xcode Command Line Tools、Python 3.9+ 和 Homebrew。第三方源码由脚本下载；
运行所需的大型数据文件通过 Git LFS 保存，克隆步骤见下文。

```sh
brew install cmake ninja sdl3 erfa libpng vulkan-tools vulkan-validationlayers
python3 scripts/bootstrap.py --all
python3 -m pip install Pillow==11.3.0 numpy==2.0.2 opencv-python-headless==4.13.0.92
python3 scripts/prepare_background.py

cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=26.0
cmake --build build -j 8

build/astra_cli --generate-cio data/time/cio.bin 4
python3 scripts/fetch_gaia.py
python3 scripts/build_catalog.py

ctest --test-dir build --output-on-failure
python3 scripts/package_macos.py
open dist/Astra.app
```

仓库中的星历、星表、银河图像和字体等二进制资源使用 Git LFS。克隆前安装并启用 LFS：

```sh
brew install git-lfs
git lfs install
git clone https://github.com/magicianA/astra.git
cd astra
git lfs pull
```

LFS 数据约 4.3 GB；普通 Git 只保存对应指针。已有完整 LFS 数据时，依赖仍按上面的构建步骤安装，
数据准备脚本会复用通过校验的本地文件。`build/`、`dist/`、`artifacts/`、第三方依赖目录和原始星表下载缓存不入库。

macOS 应用图标随构建和打包自动加入。原图与 ICNS 位于 `assets/macos/`；修改原图后，在 Mac 上运行
`python3 scripts/prepare_app_icon.py` 重新生成各尺寸图标，再构建应用即可。

DE441 约 3 GB；Gaia CSV 压缩文件约数百 MB。下载及首次数据生成需要时间，建议预留 8 GB 以上空间。
`bootstrap.py` 校验固定文件的 SHA-256，支持大型文件断点续传；Gaia 脚本复用异步作业，但中断的
CSV 传输会重新下载。EOP 快照固定于 2026-09-04；上游会更新该文件，已有快照应随源码保留。

若只构建命令行计算器，可在 CMake 配置时加 `-DASTRA_BUILD_APP=OFF`。
其他平台需要 C++20 编译器、CMake、ERFA、对应平台 CSPICE；窗口版本还需要 SDL3、Vulkan SDK、
libpng 和 glslang。CMake 支持通过 `CMAKE_PREFIX_PATH`、`CSPICE_ROOT`、`ERFA_INCLUDE_DIR`、
`ERFA_LIBRARY` 指定安装位置。各平台必须使用对应的 CSPICE 库，不能复用 Mac 二进制。
此处没有把未执行的 Windows/Linux 构建标为通过。

## 数值查询和验证

```sh
build/astra_cli --date -2500-01-01T22:00:00 --site 116.4074,39.9042,45 \
  --scale UT1 --no-atmosphere --output artifacts/ancient.json

python3 scripts/validate_horizons.py
```

CLI 导出各天体的方位、高度、视方向、距离、亮度及时间模型信息。`--scenario FILE` 可复现已保存
的场景。截图旁的 JSON 另存实际 TT/UT1/TDB、ΔT、质量说明和数据版本。

Mac Vulkan 同步验证与自动窗口回归：

```sh
DYLD_LIBRARY_PATH=/opt/homebrew/opt/vulkan-validationlayers/lib \
VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
VK_LAYER_PATH=/opt/homebrew/opt/vulkan-validationlayers/share/vulkan/explicit_layer.d \
VK_LAYER_VALIDATE_SYNC=1 \
build/astra.app/Contents/MacOS/astra --validation --smoke --frames 300 \
  --screenshot artifacts/astra-smoke.png
```

连续播放回归可使用 `--play-speed 3600 --frames 600`；负速度表示反向播放。播放中的显示复用
最近一次恒星惯性视方向，方向刷新间隔至少 150 ms，完整星表计算较慢时会延长。地球自转、
太阳、月球和行星仍按显示时刻逐帧计算；暂停和导出时恢复完整计算。高倍速下这项恒星近似的
误差会增加，不能把播放帧当作精密观测结果。

移动中的星名、行星名和方位标注保留小数像素坐标，避免文字库默认取整造成的逐像素跳动。
`ctest` 中的 `text_motion` 使用实际中英文字体，在普通和 Retina 缩放下检查连续位移及阴影对齐。
恒星使用固定宽度的紧凑光点和压缩后的亮度，避免亮星膨胀成白斑；月球等可分辨圆面按角半径绘制。

银河背景使用 [NASA Deep Star Maps 2020 的 Gaia 银河背景层](https://svs.gsfc.nasa.gov/4851/)，
原生 **16384×8192**，约为旧版像素数的 16.8 倍。采用已分离亮星的背景数据，保留暗尘带、
星群和细碎星光；用线性光 mipmap 与可选的 8× 各向异性过滤稳定缩放和拖动。
随地点和时间转动，受晨昏、月光、消光和光污染影响。“显示”中的“银河光带”
可即时开关。体验阿塔卡马无月夜可运行：

```sh
open dist/Astra.app --args --scenario "$PWD/examples/milky-way.json"
```

银河是固定在银道坐标中的遥远背景，未模拟一万年间的银河、尘埃与背景暗星演化。原始线性 EXR
经亮度压缩后编码成 sRGB PNG；保留原生像素，仅对极点行取平均，不进行模糊、锐化或放大。
亮度仍是显示近似，不是绝对辐射测量；独立恒星由星表计算。16K 纹理及 mipmap 约占 683 MiB
显存；设备不支持 16K 时自动选取受支持的较低层级。离线准备需要额外下载约 357 MiB 原始数据，
应用包只携带处理后的 PNG，不依赖 OpenCV 或联网。背景来源和处理散列另存于
`data/background/manifest.json`，场景导出记录独立的 `background_id`。

## 代码格式

C++ 和 GLSL 使用 clang-format 21：4 空格缩进、100 列、控制流加花括号。Python 使用 Ruff，
CMake 使用 cmake-format。只处理本项目源码，不重排第三方代码。

```sh
python3 -m venv .venv
.venv/bin/pip install ruff==0.16.6 cmakelang==0.6.13
PATH="$PWD/.venv/bin:$PATH" python3 scripts/format.py
PATH="$PWD/.venv/bin:$PATH" python3 scripts/format.py --check
```

`include/astro/` 定义模块接口，`src/` 是天文核心、Vulkan 渲染器和 UI，`shaders/` 存放 GLSL，
`scripts/` 负责采集、数据转换、验证、格式化和打包，`tests/` 包含数值回归与独立参考数据。

## 文档

[原始设计](docs/DESIGN.md) · [实现与验证记录](docs/IMPLEMENTATION.md) ·
[数据来源](docs/DATA_SOURCES.md) · [本机 Vulkan 环境](docs/ENVIRONMENT.md) ·
[第三方署名与许可](THIRD_PARTY_NOTICES.md)

[黄昏与黎明渲染研究及设计（英文）](docs/TWILIGHT_DESIGN.md)
