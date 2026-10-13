use anyhow::{Context, Result, bail, ensure};
use astro::{
    camera::*,
    events::scene_at,
    i18n::*,
    jobs::{
        NavigationWorker, SequenceJob, SequenceProgress, SkyWorker, comparison_scene,
        export_sequence,
    },
    math::*,
    render_scene::render_scene,
    renderer::Renderer,
    scenario::*,
    sequence::*,
    sky::*,
    time::*,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
#[derive(Clone)]
struct Options {
    data: PathBuf,
    scene: Scenario,
    validation: bool,
    language: Option<Language>,
    frames: u64,
    speed: f64,
    screenshot: Option<PathBuf>,
    hide_ui: bool,
    smoke: bool,
    headless: bool,
    width: u32,
    height: u32,
    sequence: Option<SequenceRequest>,
}
fn parse_date(text: &str, mut d: CivilDate) -> Result<CivilDate> {
    let (date, time) = text
        .split_once('T')
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let p: Vec<_> = date.rsplitn(3, '-').collect();
    ensure!(p.len() == 3, "Invalid date");
    d.year = p[2].parse()?;
    d.month = p[1].parse()?;
    d.day = p[0].parse()?;
    if let Some(time) = time {
        let p: Vec<_> = time.split(':').collect();
        ensure!(p.len() == 3, "Invalid time");
        d.hour = p[0].parse()?;
        d.minute = p[1].parse()?;
        d.second = p[2].parse()?;
    }
    Ok(d)
}
fn options() -> Result<Option<Options>> {
    let mut o = Options {
        data: astro::source_data(),
        scene: Scenario::default(),
        validation: false,
        language: None,
        frames: 0,
        speed: 0.,
        screenshot: None,
        hide_ui: false,
        smoke: false,
        headless: false,
        width: 1280,
        height: 800,
        sequence: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("Missing argument for {arg}"))
        };
        match arg.as_str() {
            "--data" => o.data = value()?.into(),
            "--scenario" => o.scene = load_scenario(value()?, "")?,
            "--date" => o.scene.date = parse_date(&value()?, o.scene.date)?,
            "--year" => o.scene.date.year = value()?.parse()?,
            "--validation" => o.validation = true,
            "--language" => {
                o.language =
                    Some(parse_language(&value()?).context("Supported UI languages: en, zh-CN")?)
            }
            "--frames" => o.frames = value()?.parse()?,
            "--play-speed" => o.speed = value()?.parse()?,
            "--screenshot" => o.screenshot = Some(value()?.into()),
            "--hide-ui" => o.hide_ui = true,
            "--smoke" => o.smoke = true,
            "--headless" => o.headless = true,
            "--width" => o.width = value()?.parse()?,
            "--height" => o.height = value()?.parse()?,
            "--sequence" => {
                o.sequence.get_or_insert_with(Default::default).directory = value()?.into()
            }
            "--steps" => {
                o.sequence.get_or_insert_with(Default::default).frames = value()?.parse()?
            }
            "--step" => {
                o.sequence.get_or_insert_with(Default::default).step_seconds = value()?.parse()?
            }
            "--fps" => o.sequence.get_or_insert_with(Default::default).fps = value()?.parse()?,
            "--no-video" => o.sequence.get_or_insert_with(Default::default).video = false,
            "--trails" => o.sequence.get_or_insert_with(Default::default).trails = true,
            "--unlocked-exposure" => {
                o.sequence
                    .get_or_insert_with(Default::default)
                    .lock_exposure = false
            }
            "--camera-end" => {
                let r = o.sequence.get_or_insert_with(Default::default);
                r.camera_end = load_scenario(value()?, "")?;
                r.camera_path = true;
            }
            "--shaders" => {
                let _ = value()?;
            }
            "--help" | "-h" => {
                println!(
                    "Astra: --data DIR --scenario FILE --date YEAR-MM-DDTHH:MM:SS --year YEAR --validation --frames N --play-speed RATE --screenshot FILE.png --hide-ui --smoke --language en|zh-CN\nOffscreen: --headless --width N --height N\nSequence: --sequence EMPTY_DIR --steps N --step SECONDS --fps N --no-video --trails --camera-end SCENE.json --unlocked-exposure"
                );
                return Ok(None);
            }
            _ => bail!("Unknown argument: {arg}"),
        }
    }
    ensure!(
        o.speed.is_finite() && o.speed.abs() <= 31557600.,
        "Playback speed must be finite and at most one year per second"
    );
    ensure!(
        o.width > 0 && o.height > 0 && o.width <= 16384 && o.height <= 16384,
        "Invalid output dimensions"
    );
    validate(&o.scene)?;
    if let Some(r) = &o.sequence {
        validate_sequence(r)?;
        ensure!(
            !o.smoke && o.frames == 0 && o.speed == 0. && o.screenshot.is_none(),
            "Sequence export cannot be combined with smoke, frames, playback or screenshot"
        );
    }
    Ok(Some(o))
}
fn save_image(
    path: &Path,
    image: &image::RgbaImage,
    s: &Scenario,
    sky: &SkySnapshot,
    exposure: f32,
) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    save_scenario(
        s,
        path.with_extension("json"),
        &sky.data_id,
        Some(&sky.time),
        exposure as f64,
    )
}
fn make_image(
    engine: &SkyEngine,
    renderer: &mut Renderer,
    s: &Scenario,
    width: u32,
    height: u32,
    dt: f64,
) -> Result<(image::RgbaImage, SkySnapshot)> {
    let sky = engine.compute(s, 0, None, Scope::FullSky, None)?;
    let camera = s.camera(
        if s.compare {
            (width / 2) as f64
        } else {
            width as f64
        },
        height as f64,
    );
    let scene = render_scene(&sky, &sky, s, &camera, Some(&engine.catalog));
    let comparison = if let Some(cs) = comparison_scene(s) {
        let compare = engine.compute(&cs, 0, None, Scope::FullSky, None)?;
        let camera = cs.camera((width - width / 2) as f64, height as f64);
        Some(render_scene(
            &compare,
            &compare,
            &cs,
            &camera,
            Some(&engine.catalog),
        ))
    } else {
        None
    };
    let mut image = renderer.render(&scene, comparison.as_ref(), width, height, dt)?;
    if s.adaptive_exposure {
        image = renderer.render(&scene, comparison.as_ref(), width, height, dt)?;
    }
    Ok((image, sky))
}
fn user_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Astra/Sky")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("Astra/Sky")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("astra/sky")
    }
}
struct App {
    options: Options,
    engine: Arc<SkyEngine>,
    renderer: Renderer,
    worker: SkyWorker,
    navigation: NavigationWorker,
    sequence_job: Option<SequenceJob>,
    navigation_scene: Option<Scenario>,
    navigation_moon: bool,
    sky: SkySnapshot,
    frame: SkySnapshot,
    comparison: Option<SkySnapshot>,
    comparison_frame: Option<SkySnapshot>,
    scene: Scenario,
    language: Language,
    user: PathBuf,
    visible: bool,
    playing: bool,
    speed: f64,
    selected: Option<Object>,
    track: bool,
    search: String,
    notice: String,
    texture: Option<egui::TextureHandle>,
    last_scene: Scenario,
    last_time: Instant,
    last_compute: Instant,
    last_render: Option<(Scenario, u32, u32)>,
    frames: u64,
    pending_shot: Option<PathBuf>,
    path: String,
    exploration: astro::explore::Exploration,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        o: Options,
        engine: Arc<SkyEngine>,
        renderer: Renderer,
    ) -> Result<Self> {
        let sky = engine.compute(&o.scene, 0, None, Scope::FullSky, None)?;
        let user = user_dir();
        let fallback = std::env::var("LANG")
            .ok()
            .and_then(|s| parse_language(&s))
            .unwrap_or(Language::English);
        let language = o
            .language
            .unwrap_or_else(|| load_language(user.join("ui-preferences.json"), fallback));
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read(o.data.join("fonts/NotoSansCJKsc-Regular.otf")) {
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("cjk".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let scene = o.scene.clone();
        let worker = SkyWorker::new(engine.clone());
        let navigation = NavigationWorker::new(engine.clone());
        let comparison = if let Some(cs) = comparison_scene(&scene) {
            Some(engine.compute(&cs, 0, None, Scope::FullSky, None)?)
        } else {
            None
        };
        let mut exploration = astro::explore::Exploration::default();
        exploration.language = language;
        Ok(Self {
            visible: !o.hide_ui,
            playing: o.speed != 0.,
            speed: if o.speed == 0. { 60. } else { o.speed },
            pending_shot: o.screenshot.clone(),
            last_scene: scene.clone(),
            last_time: Instant::now(),
            last_compute: Instant::now(),
            last_render: None,
            options: o,
            engine,
            renderer,
            worker,
            navigation,
            sequence_job: None,
            navigation_scene: None,
            navigation_moon: false,
            comparison_frame: comparison.clone(),
            frame: sky.clone(),
            sky,
            comparison,
            scene,
            language,
            user,
            selected: None,
            track: false,
            search: String::new(),
            notice: String::new(),
            texture: None,
            frames: 0,
            path: String::new(),
            exploration,
        })
    }
    fn tr<'a>(&self, en: &'a str, zh: &'a str) -> &'a str {
        if self.language == Language::Chinese {
            zh
        } else {
            en
        }
    }
    fn focus(&mut self, o: Object) {
        self.scene.azimuth = o.azimuth() / RAD;
        self.scene.elevation = o.altitude() / RAD;
        self.scene.roll = 0.;
        self.selected = Some(o);
        self.track = true;
    }
    fn seek(&mut self, twilight: i32) {
        self.playing = false;
        self.navigation_moon = twilight == 0;
        if !self.navigation_moon {
            self.track = false;
        }
        self.navigation.request(&self.scene, twilight);
        self.navigation_scene = Some(self.scene.clone());
        self.notice = self.tr("Searching…", "正在搜索…").into();
    }
    fn controls(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let tr = |en, zh| {
            if language == Language::Chinese {
                zh
            } else {
                en
            }
        };

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Astra");
                ui.label(tr("Search", "搜索"));
                ui.text_edit_singleline(&mut self.search);
                if ui.button("中文 / English").clicked() {
                    self.language = if self.language == Language::English {
                        Language::Chinese
                    } else {
                        Language::English
                    };
                    self.exploration.language = self.language;
                    if let Err(e) =
                        save_language(self.user.join("ui-preferences.json"), self.language)
                    {
                        self.notice = e.to_string();
                    }
                }
                if ui.button(tr("Hide UI · H", "隐藏界面 · H")).clicked() {
                    self.visible = false;
                }
            });
            if !self.search.trim().is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for o in self.frame.bodies.clone() {
                        let name = body_name(o.body);
                        let alias = chinese_body(o.body);
                        if name.to_lowercase().contains(&self.search.to_lowercase())
                            || alias.contains(self.search.trim())
                        {
                            if ui
                                .button(if self.language == Language::Chinese {
                                    alias
                                } else {
                                    &name
                                })
                                .clicked()
                            {
                                self.focus(o);
                            }
                        }
                    }
                    for index in self.engine.catalog.search(&self.search, 10) {
                        let star = self.engine.catalog.stars[index as usize];
                        if ui.button(self.engine.catalog.name(&star)).clicked() {
                            match self.engine.compute(
                                &self.scene,
                                0,
                                None,
                                Scope::FullSky,
                                Some(index),
                            ) {
                                Ok(sky) => {
                                    if let Some(&o) = sky.stars.first() {
                                        self.focus(o);
                                    }
                                }
                                Err(e) => self.notice = e.to_string(),
                            }
                        }
                    }
                });
            }
        });
        egui::SidePanel::left("settings")
            .default_width(255.)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.collapsing(tr("Location", "地点"), |ui| {
                        ui.text_edit_singleline(&mut self.scene.location_name);
                        number(
                            ui,
                            tr("Longitude °", "经度 °"),
                            &mut self.scene.longitude,
                            0.01,
                        );
                        number(
                            ui,
                            tr("Latitude °", "纬度 °"),
                            &mut self.scene.latitude,
                            0.01,
                        );
                        number(ui, tr("Elevation m", "海拔 m"), &mut self.scene.height, 1.);
                        for (name, lon, lat, h) in [
                            ("Beijing 北京", 116.4074, 39.9042, 45.),
                            ("Atacama 阿塔卡马", -70.4, -24.6, 2400.),
                            ("Greenwich 格林尼治", 0., 51.4779, 46.),
                        ] {
                            if ui.button(name).clicked() {
                                self.scene.longitude = lon;
                                self.scene.latitude = lat;
                                self.scene.height = h;
                                self.scene.location_name = name.into();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button(tr("Save location", "收藏地点")).clicked() {
                            match save_scenario(
                                &self.scene,
                                self.user.join("favorite.json"),
                                "",
                                None,
                                0.0,
                            ) {
                                Ok(()) => self.notice = tr("Location saved", "地点已收藏").into(),
                                Err(e) => self.notice = e.to_string(),
                            }
                        }
                        if ui.button(tr("Restore location", "恢复收藏")).clicked() {
                            match load_scenario(self.user.join("favorite.json"), "") {
                                Ok(s) => {
                                    self.scene.longitude = s.longitude;
                                    self.scene.latitude = s.latitude;
                                    self.scene.height = s.height;
                                    self.scene.location_name = s.location_name;
                                }
                                Err(e) => self.notice = e.to_string(),
                            }
                        }
                    });
                    ui.collapsing(tr("Time", "时间"), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(tr("Date", "日期"));
                            ui.add(
                                egui::DragValue::new(&mut self.scene.date.year).range(-3000..=6999),
                            );
                            ui.add(egui::DragValue::new(&mut self.scene.date.month).range(1..=12));
                            ui.add(egui::DragValue::new(&mut self.scene.date.day).range(1..=31));
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut self.scene.date.hour).range(0..=23));
                            ui.label(":");
                            ui.add(egui::DragValue::new(&mut self.scene.date.minute).range(0..=59));
                            ui.label(":");
                            ui.add(
                                egui::DragValue::new(&mut self.scene.date.second).range(0.0..=60.0),
                            );
                        });
                        egui::ComboBox::from_id_salt("scale")
                            .selected_text(scale_name(self.scene.scale))
                            .show_ui(ui, |ui| {
                                for scale in [
                                    TimeScale::UT1,
                                    TimeScale::TT,
                                    TimeScale::TDB,
                                    TimeScale::UTC,
                                    TimeScale::LocalMean,
                                ] {
                                    ui.selectable_value(
                                        &mut self.scene.scale,
                                        scale,
                                        scale_name(scale),
                                    );
                                }
                            });
                        let label = tr("Julian calendar", "儒略历");
                        ui.checkbox(&mut self.scene.julian, label);
                        let label = tr("Custom ΔT", "自定义 ΔT");
                        ui.checkbox(&mut self.scene.override_delta_t, label);
                        if self.scene.override_delta_t {
                            number(ui, "ΔT (s)", &mut self.scene.custom_delta_t, 1.);
                        }
                        if ui.button(tr("Now", "现在")).clicked() {
                            if let Ok(t) =
                                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                            {
                                self.scene.date = from_jd(
                                    JulianDate::from(2440587.5 + t.as_secs_f64() / 86400.),
                                    false,
                                );
                                self.scene.scale = TimeScale::UTC;
                                self.scene.julian = false;
                            }
                        }
                        if ui
                            .button(tr("Restore day & night", "恢复昼夜效果"))
                            .clicked()
                        {
                            restore_day_night(&mut self.scene);
                        }
                        ui.horizontal(|ui| {
                            for (label, event) in [
                                (tr("Prev dawn", "前次黎明"), -1),
                                (tr("Next dawn", "下次黎明"), 1),
                            ] {
                                if ui.button(label).clicked() {
                                    self.seek(event);
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            for (label, event) in [
                                (tr("Prev dusk", "前次黄昏"), -2),
                                (tr("Next dusk", "下次黄昏"), 2),
                            ] {
                                if ui.button(label).clicked() {
                                    self.seek(event);
                                }
                            }
                        });
                        ui.add(
                            egui::Slider::new(&mut self.scene.date.year, -3000..=6999)
                                .text(tr("Year", "年份")),
                        );
                    });
                    ui.collapsing(tr("View", "视图"), |ui| {
                        for (en, zh, value) in [
                            ("Atmosphere", "大气", &mut self.scene.atmosphere),
                            ("Ground", "地面", &mut self.scene.ground),
                            ("Milky Way", "银河", &mut self.scene.milky_way),
                            ("Grid", "网格", &mut self.scene.grid),
                            ("Labels", "标签", &mut self.scene.labels),
                            ("Lunar surface", "月面", &mut self.scene.moon_surface),
                            ("Earthshine", "地球照", &mut self.scene.earthshine),
                        ] {
                            ui.checkbox(
                                value,
                                if self.language == Language::Chinese {
                                    zh
                                } else {
                                    en
                                },
                            );
                        }
                        egui::ComboBox::from_id_salt("projection")
                            .selected_text(match self.scene.projection {
                                ProjectionKind::Perspective => tr("Perspective", "透视"),
                                ProjectionKind::Fisheye => tr("Fisheye", "鱼眼"),
                                ProjectionKind::Stereographic => tr("Stereographic", "立体投影"),
                            })
                            .show_ui(ui, |ui| {
                                for (name, kind) in [
                                    ("Perspective 透视", ProjectionKind::Perspective),
                                    ("Fisheye 鱼眼", ProjectionKind::Fisheye),
                                    ("Stereographic 立体投影", ProjectionKind::Stereographic),
                                ] {
                                    if ui
                                        .selectable_value(&mut self.scene.projection, kind, name)
                                        .changed()
                                    {
                                        self.scene.fov =
                                            self.scene.fov.min(maximum_zoom_fov(kind) / RAD);
                                    }
                                }
                            });
                        ui.add(
                            egui::Slider::new(
                                &mut self.scene.fov,
                                1.0..=maximum_zoom_fov(self.scene.projection) / RAD,
                            )
                            .text("FOV °"),
                        );
                        if ui.button(tr("Panorama", "全景")).clicked() {
                            self.scene.projection = ProjectionKind::Perspective;
                            self.scene.fov = 60.;
                            self.scene.roll = 0.;
                        }
                        ui.add(
                            egui::Slider::new(&mut self.scene.magnitude, -2.0..=12.0)
                                .text(tr("Magnitude limit", "极限星等")),
                        );
                        let label = tr("Automatic exposure", "自动曝光");
                        ui.checkbox(&mut self.scene.auto_exposure, label);
                        let label = tr("Adaptive exposure", "自适应曝光");
                        if ui
                            .checkbox(&mut self.scene.adaptive_exposure, label)
                            .changed()
                        {
                            self.scene.exposure = 1.;
                        }
                        let mut ev = self.scene.exposure.log2();
                        if ui
                            .add(egui::Slider::new(&mut ev, -8.0..=8.0).text("EV"))
                            .changed()
                        {
                            self.scene.exposure = ev.exp2();
                        }
                        ui.add(
                            egui::Slider::new(&mut self.scene.light_pollution, 0.0..=1.0)
                                .text(tr("Light pollution", "光污染")),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.scene.extinction, 0.0..=2.0)
                                .text(tr("Extinction", "消光")),
                        );
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut self.scene.atmosphere_preset,
                                0,
                                tr("Clear", "晴朗"),
                            );
                            ui.selectable_value(
                                &mut self.scene.atmosphere_preset,
                                1,
                                tr("Hazy", "雾霾"),
                            );
                        });
                        number(
                            ui,
                            tr("Pressure hPa", "气压 hPa"),
                            &mut self.scene.pressure,
                            1.,
                        );
                        number(
                            ui,
                            tr("Temperature °C", "温度 °C"),
                            &mut self.scene.temperature,
                            1.,
                        );
                        ui.text_edit_singleline(&mut self.path);
                        ui.horizontal(|ui| {
                            if ui.button(tr("Save scene", "保存场景")).clicked() {
                                let path = if self.path.is_empty() {
                                    self.user.join("scene.json")
                                } else {
                                    PathBuf::from(&self.path)
                                };
                                match save_scenario(
                                    &self.scene,
                                    &path,
                                    &self.sky.data_id,
                                    Some(&self.frame.time),
                                    self.renderer.effective_exposure as f64,
                                ) {
                                    Ok(()) => self.notice = format!("{}", path.display()),
                                    Err(e) => self.notice = e.to_string(),
                                }
                            }
                            if ui.button(tr("Load", "加载")).clicked() {
                                let path = if self.path.is_empty() {
                                    self.user.join("scene.json")
                                } else {
                                    PathBuf::from(&self.path)
                                };
                                match load_scenario(&path, &self.sky.data_id) {
                                    Ok(s) => self.scene = s,
                                    Err(e) => self.notice = e.to_string(),
                                }
                            }
                        });
                        if ui.button(tr("Export PNG", "导出 PNG")).clicked() {
                            self.pending_shot = Some(if self.path.is_empty() {
                                self.user.join("sky.png")
                            } else {
                                PathBuf::from(&self.path).with_extension("png")
                            });
                        }
                    });
                    ui.collapsing(tr("Moon", "月球"), |ui| {
                        if let Some(o) = self.frame.bodies.iter().find(|o| o.body == 301).copied() {
                            ui.label(format!(
                                "{} {:.1}° · {:.1}% · {:.0} km",
                                tr("Altitude", "高度"),
                                o.altitude() / RAD,
                                o.phase * 100.,
                                o.distance_au * AU_KM
                            ));
                            if ui.button(tr("Focus Moon", "定位月球")).clicked() {
                                self.focus(o);
                            }
                            if ui.button(tr("Zoom to Moon", "放大月球")).clicked() {
                                self.focus(o);
                                self.scene.fov = 1.;
                            }
                        }
                        if ui
                            .button(tr("Find a viewing time", "寻找观测时刻"))
                            .clicked()
                        {
                            self.seek(0);
                        }
                    });
                    ui.collapsing(tr("Explore", "探索"), |ui| {
                        match self.exploration.draw(
                            ui,
                            &mut self.scene,
                            self.engine.clone(),
                            Some(&self.frame),
                            self.selected,
                            &self.options.data,
                            &self.user,
                        ) {
                            Ok(_) => {}
                            Err(e) => self.notice = e.to_string(),
                        }
                    });
                    if let Some(o) = self.selected {
                        ui.separator();
                        ui.heading(if o.body != 0 {
                            if self.language == Language::Chinese {
                                chinese_body(o.body).into()
                            } else {
                                body_name(o.body).to_owned()
                            }
                        } else {
                            self.engine
                                .catalog
                                .name(&self.engine.catalog.stars[o.catalog_index as usize])
                        });
                        ui.label(format!(
                            "Az {:.4}° · Alt {:.4}°",
                            o.azimuth() / RAD,
                            o.altitude() / RAD
                        ));
                        ui.label(format!(
                            "Magnitude {:.2} · {:.6} AU",
                            o.magnitude, o.distance_au
                        ));
                        ui.label(
                            Translator {
                                language: self.language,
                            }
                            .message(&quality_text(&o))
                            .as_ref(),
                        );
                        let label = tr("Track", "跟踪");
                        ui.checkbox(&mut self.track, label);
                    }
                    ui.separator();
                    ui.label(&self.notice);
                    ui.small(format!(
                        "{} · {} stars · {:.0} ms",
                        self.renderer.gpu.name,
                        self.sky.stars.len(),
                        self.sky.compute_ms
                    ));
                    ui.small(&self.frame.time.note);
                });
            });
        egui::TopBottomPanel::bottom("playback").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                    self.playing = !self.playing;
                }
                for (label, step) in [
                    ("−day", -86400.),
                    ("−hour", -3600.),
                    ("+hour", 3600.),
                    ("+day", 86400.),
                ] {
                    if ui.button(label).clicked() {
                        match scene_at(&self.scene, step) {
                            Ok(s) => self.scene = s,
                            Err(e) => self.notice = e.to_string(),
                        }
                    }
                }
                ui.add(
                    egui::DragValue::new(&mut self.speed)
                        .speed(60.)
                        .prefix("× "),
                );
                ui.label(format!(
                    "{} {}",
                    format_date(&self.scene.date),
                    scale_name(self.scene.scale)
                ));
            });
        });
    }
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64, speed: f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(speed));
    });
}
fn chinese_body(id: i32) -> &'static str {
    match id {
        10 => "太阳",
        301 => "月球",
        199 => "水星",
        299 => "金星",
        4 => "火星",
        5 => "木星",
        6 => "土星",
        7 => "天王星",
        8 => "海王星",
        _ => "天体",
    }
}
fn astronomy_changed(a: &Scenario, b: &Scenario) -> bool {
    a.date != b.date
        || a.scale != b.scale
        || a.longitude != b.longitude
        || a.latitude != b.latitude
        || a.height != b.height
        || a.julian != b.julian
        || a.override_delta_t != b.override_delta_t
        || a.custom_delta_t != b.custom_delta_t
        || a.magnitude != b.magnitude
        || a.atmosphere != b.atmosphere
        || a.pressure != b.pressure
        || a.temperature != b.temperature
        || a.earthshine != b.earthshine
        || a.constellations != b.constellations
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let was_playing = self.playing;
        let now = Instant::now();
        let dt = (now - self.last_time).as_secs_f64().min(0.1);
        self.last_time = now;
        while let Ok(result) = self.worker.receiver.try_recv() {
            match result {
                Ok(pair) => {
                    self.sky = pair.primary;
                    self.comparison = pair.comparison;
                }
                Err(e) => self.notice = e.to_string(),
            }
        }
        while let Ok(result) = self.navigation.receiver.try_recv() {
            if self
                .navigation_scene
                .take()
                .is_some_and(|s| !astronomy_changed(&s, &self.scene))
            {
                match result {
                    Ok(Some(s)) => {
                        self.scene = s;
                        self.notice.clear();
                        if self.navigation_moon {
                            match self.engine.compute(
                                &self.scene,
                                0,
                                None,
                                Scope::SolarSystem,
                                None,
                            ) {
                                Ok(sky) => {
                                    self.selected =
                                        sky.bodies.iter().find(|o| o.body == 301).copied();
                                    self.track = self.selected.is_some();
                                }
                                Err(e) => self.notice = e.to_string(),
                            }
                        } else {
                            self.selected = None;
                            self.track = false;
                        }
                    }
                    Ok(None) => {
                        self.notice = self
                            .tr(
                                "No suitable time within the data range",
                                "数据范围内未找到合适时刻",
                            )
                            .into()
                    }
                    Err(e) => self.notice = e.to_string(),
                }
            }
        }
        let sequence_updates = self
            .sequence_job
            .as_ref()
            .map(|job| job.receiver.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for update in sequence_updates {
            match update {
                SequenceProgress::Frame(count) => self.exploration.completed_frames = count,
                SequenceProgress::Finished(result) => {
                    self.sequence_job = None;
                    self.exploration.recording = false;
                    self.notice = match result {
                        Ok(()) => self.tr("Sequence exported", "序列导出完成").into(),
                        Err(e) => e.to_string(),
                    };
                }
            }
        }
        if self.exploration.cancel_recording {
            if let Some(job) = &self.sequence_job {
                job.cancel();
            }
            self.exploration.cancel_recording = false;
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::H)) {
                self.visible = !self.visible;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.playing = !self.playing;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.track = false;
                self.selected = None;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.scene.roll = 0.;
            }
            ctx.input(|i| {
                let step = dt * 30.;
                if i.key_down(egui::Key::ArrowLeft) {
                    self.scene.azimuth -= step;
                }
                if i.key_down(egui::Key::ArrowRight) {
                    self.scene.azimuth += step;
                }
                if i.key_down(egui::Key::ArrowUp) {
                    self.scene.elevation = (self.scene.elevation + step).min(90.);
                }
                if i.key_down(egui::Key::ArrowDown) {
                    self.scene.elevation = (self.scene.elevation - step).max(-90.);
                }
            });
        }
        if self.playing {
            match scene_at(&self.scene, self.speed * dt) {
                Ok(s) => self.scene = s,
                Err(e) => {
                    self.notice = e.to_string();
                    self.playing = false;
                }
            }
        }
        if self.visible {
            self.controls(ctx);
        }
        if let Err(e) = validate(&self.scene) {
            self.notice = e.to_string();
            self.playing = false;
            return;
        }
        if let Some(notice) = self.exploration.notice.take() {
            self.notice = notice;
        }
        if let Some((object, play)) = self.exploration.pending_selection.take() {
            self.selected = Some(object);
            self.track = true;
            if play {
                self.playing = true;
                self.speed = 120.0;
            }
        }
        if was_playing && !self.playing {
            self.worker.request(&self.scene);
        }
        let changed = astronomy_changed(&self.scene, &self.last_scene);
        if self
            .navigation_scene
            .as_ref()
            .is_some_and(|origin| astronomy_changed(origin, &self.scene))
        {
            self.navigation.cancel();
            self.navigation_scene = None;
        }
        let compare_changed = self.scene.compare != self.last_scene.compare
            || self.scene.comparison_year != self.last_scene.comparison_year;
        if changed {
            match self
                .engine
                .compute(&self.scene, 0, None, Scope::SolarSystem, None)
            {
                Ok(frame) => self.frame = frame,
                Err(e) => {
                    self.notice = e.to_string();
                    self.playing = false;
                    return;
                }
            }
            let interval = (self.sky.compute_ms / 1000. * 1.2).max(0.15);
            if !self.playing || self.last_compute.elapsed().as_secs_f64() >= interval {
                self.worker.request(&self.scene);
                self.last_compute = now;
            }
        }
        if compare_changed && !changed {
            self.worker.request(&self.scene);
        }
        if let Some(cs) = comparison_scene(&self.scene) {
            if self
                .comparison_frame
                .as_ref()
                .is_none_or(|sky| astronomy_changed(&sky.scenario, &cs))
            {
                match self.engine.compute(&cs, 0, None, Scope::SolarSystem, None) {
                    Ok(sky) => self.comparison_frame = Some(sky),
                    Err(e) => self.notice = e.to_string(),
                }
            }
        } else {
            self.comparison = None;
            self.comparison_frame = None;
        }
        if let Some(selected) = self.selected {
            let current = if selected.body != 0 {
                self.frame
                    .bodies
                    .iter()
                    .find(|o| o.body == selected.body)
                    .copied()
            } else {
                let cached = self
                    .sky
                    .stars
                    .iter()
                    .find(|o| o.id == selected.id)
                    .unwrap_or(&selected);
                Some(observe_star(cached, &self.frame))
            };
            if let Some(o) = current {
                self.selected = Some(o);
                if self.track {
                    self.scene.azimuth = o.azimuth() / RAD;
                    self.scene.elevation = o.altitude() / RAD;
                    self.scene.roll = 0.;
                }
            }
        }
        self.frame.scenario = self.scene.clone();
        self.last_scene = self.scene.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let available = ui.available_size();
                let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
                let ppp = ctx.pixels_per_point();
                let width = (rect.width() * ppp).round().max(2.) as u32;
                let height = (rect.height() * ppp).round().max(2.) as u32;
                let camera = self.scene.camera(
                    if self.scene.compare {
                        (width / 2) as f64
                    } else {
                        width as f64
                    },
                    height as f64,
                );
                if response.dragged() {
                    let delta = ctx.input(|i| i.pointer.delta()) * ppp;
                    let mut camera = camera;
                    camera.pan(0., 0., delta.x as f64, delta.y as f64);
                    self.scene.azimuth = camera.azimuth / RAD;
                    self.scene.elevation = camera.elevation / RAD;
                    self.scene.roll = 0.;
                    self.track = false;
                }
                if response.hovered() {
                    let wheel = ctx.input(|i| i.raw_scroll_delta.y);
                    if wheel != 0. {
                        self.scene.fov = zoom_fov(
                            self.scene.fov * RAD,
                            self.scene.projection,
                            wheel as f64 / 40.,
                        ) / RAD;
                    }
                }
                let camera = self.scene.camera(
                    if self.scene.compare {
                        (width / 2) as f64
                    } else {
                        width as f64
                    },
                    height as f64,
                );
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let x = (pos.x - rect.min.x) * ppp;
                        let y = (pos.y - rect.min.y) * ppp;
                        // The comparison viewport is for synchronized exploration;
                        // object selection belongs to the primary epoch.
                        if !self.scene.compare || x < (width / 2) as f32 {
                            if let Some(object) = pick_sky_object(
                                &self.frame,
                                &self.sky,
                                &self.scene,
                                &camera,
                                x as f64,
                                y as f64,
                                18.0 * ppp as f64,
                            ) {
                                self.selected = Some(object);
                                self.track = false;
                            }
                        }
                    }
                }
                let render_needed = self
                    .last_render
                    .as_ref()
                    .is_none_or(|(s, w, h)| s != &self.scene || *w != width || *h != height)
                    || self.sky.generation != 0
                    || self.scene.adaptive_exposure
                    || self.pending_shot.is_some();
                if render_needed {
                    let s = render_scene(
                        &self.frame,
                        &self.sky,
                        &self.scene,
                        &camera,
                        Some(&self.engine.catalog),
                    );
                    let comparison = if let Some(cs) = comparison_scene(&self.scene) {
                        self.comparison.as_ref().map(|c| {
                            render_scene(
                                self.comparison_frame.as_ref().unwrap_or(c),
                                c,
                                &cs,
                                &cs.camera((width - width / 2) as f64, height as f64),
                                Some(&self.engine.catalog),
                            )
                        })
                    } else {
                        None
                    };
                    match self
                        .renderer
                        .render(&s, comparison.as_ref(), width, height, dt)
                    {
                        Ok(image) => {
                            let color = egui::ColorImage::from_rgba_unmultiplied(
                                [width as usize, height as usize],
                                image.as_raw(),
                            );
                            if let Some(texture) = self.texture.as_mut() {
                                texture.set(color, egui::TextureOptions::LINEAR);
                            } else {
                                self.texture = Some(ctx.load_texture(
                                    "sky",
                                    color,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.last_render = Some((self.scene.clone(), width, height));
                            self.sky.generation = 0;
                            if (self.options.frames == 0 || self.frames + 1 >= self.options.frames)
                                && self.pending_shot.is_some()
                            {
                                let path = self.pending_shot.take().unwrap();
                                let result = make_image(
                                    &self.engine,
                                    &mut self.renderer,
                                    &self.scene,
                                    self.options.width,
                                    self.options.height,
                                    dt,
                                )
                                .and_then(|(image, sky)| {
                                    save_image(
                                        &path,
                                        &image,
                                        &self.scene,
                                        &sky,
                                        self.renderer.effective_exposure,
                                    )
                                });
                                self.notice = match result {
                                    Ok(()) => format!("{}", path.display()),
                                    Err(e) => e.to_string(),
                                };
                            }
                        }
                        Err(e) => self.notice = e.to_string(),
                    }
                }
                if let Some(texture) = &self.texture {
                    ui.painter().image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                        Color32::WHITE,
                    );
                }
                let split_x = rect.left() + (width / 2) as f32 / ppp;
                let left_rect = if self.scene.compare {
                    Rect::from_min_max(rect.min, Pos2::new(split_x, rect.bottom()))
                } else {
                    rect
                };
                draw_sky_overlay(
                    ui.painter(),
                    &self.frame,
                    &self.sky,
                    &self.scene,
                    &camera,
                    left_rect,
                    ppp,
                    &self.engine.catalog,
                    self.language,
                    self.selected,
                );
                if self.scene.compare {
                    if let Some(cached) = self.comparison.as_ref() {
                        let frame = self.comparison_frame.as_ref().unwrap_or(cached);
                        let view = comparison_scene(&self.scene).unwrap();
                        let right_camera = view.camera((width - width / 2) as f64, height as f64);
                        let right_rect =
                            Rect::from_min_max(Pos2::new(split_x, rect.top()), rect.max);
                        draw_sky_overlay(
                            ui.painter(),
                            frame,
                            cached,
                            &view,
                            &right_camera,
                            right_rect,
                            ppp,
                            &self.engine.catalog,
                            self.language,
                            None,
                        );
                    }
                }
            });
        if let Some(request) = self.exploration.pending_recording.take() {
            if self.sequence_job.is_none() {
                self.exploration.recording = true;
                self.exploration.completed_frames = 0;
                self.sequence_job = Some(SequenceJob::new(
                    self.engine.clone(),
                    self.options.data.clone(),
                    self.scene.clone(),
                    request,
                    self.options.width,
                    self.options.height,
                    self.options.validation,
                ));
            }
        }
        self.frames += 1;
        if self.options.frames > 0 && self.frames >= self.options.frames {
            if self.renderer.gpu.errors() > 0 {
                eprintln!("Vulkan validation errors: {}", self.renderer.gpu.errors());
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.playing || self.options.frames > 0 || self.scene.adaptive_exposure {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
fn pick_sky_object(
    frame: &SkySnapshot,
    cached: &SkySnapshot,
    view: &Scenario,
    camera: &Camera,
    x: f64,
    y: f64,
    mut maximum_distance: f64,
) -> Option<Object> {
    if !camera.contains(x, y) || (view.ground && !view.horizon.visible(camera.unproject(x, y), 0.0))
    {
        return None;
    }
    let projection = camera.prepare();
    let mut nearest = None;
    for source in cached.stars.iter().chain(frame.bodies.iter()) {
        let object = if source.body == 0 {
            observe_star(source, frame)
        } else {
            *source
        };
        if (object.body == 0 && object.magnitude > view.magnitude)
            || (view.ground && !view.horizon.visible(object.observed, object.angular_radius))
        {
            continue;
        }
        let point = projection.project(object.observed, object.angular_radius);
        if !point.visible {
            continue;
        }
        let distance = ((point.x - x).hypot(point.y - y)
            - object.angular_radius * projection.maximum_scale(point))
        .max(0.0);
        if distance < maximum_distance {
            nearest = Some(object);
            maximum_distance = distance;
        }
    }
    nearest
}

fn draw_sky_overlay(
    painter: &egui::Painter,
    frame: &SkySnapshot,
    cached: &SkySnapshot,
    view: &Scenario,
    camera: &Camera,
    rect: Rect,
    ppp: f32,
    catalog: &astro::catalog::Catalog,
    language: Language,
    selected: Option<Object>,
) {
    // Each epoch owns its viewport, including labels near the shared seam.
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    if view.grid {
        draw_grid(&painter, camera, rect, ppp);
    }
    let font = egui::FontId::proportional(13.0);
    let tr = Translator { language };
    let label = |object: Object, offset: f32, color: Color32, name: &str| {
        if view.ground && !view.horizon.visible(object.observed, object.angular_radius) {
            return;
        }
        let point = camera.project(object.observed);
        if point.visible {
            astro::text::draw_moving_text(
                &painter,
                Pos2::new(
                    rect.left() + point.x as f32 / ppp + offset,
                    rect.top() + point.y as f32 / ppp,
                ),
                color,
                font.clone(),
                tr.translate(name),
            );
        }
    };
    if view.labels {
        for object in &frame.bodies {
            let name = if language == Language::Chinese {
                chinese_body(object.body).to_owned()
            } else {
                body_name(object.body).to_owned()
            };
            label(*object, 8.0, Color32::from_rgb(225, 230, 240), &name);
        }
        for &index in &cached.label_stars {
            let Some(star) = cached.stars.get(index) else {
                continue;
            };
            let object = observe_star(star, frame);
            let Some(source) = catalog.stars.get(object.catalog_index as usize) else {
                continue;
            };
            label(object, 6.0, Color32::LIGHT_GRAY, &catalog.name(source));
        }
    }
    // Figure names follow each epoch's guide stars, independently of ordinary labels.
    if view.constellations && view.constellation_labels {
        for figure in &catalog.constellations {
            let centre = figure
                .segments
                .iter()
                .flatten()
                .filter_map(|hip| cached.guide_stars.get(hip))
                .map(|star| observe_star(star, frame).observed)
                .fold(Vec3::default(), |sum, direction| sum + direction);
            if norm(centre) < 1e-8 {
                continue;
            }
            let direction = unit(centre);
            if view.ground && !view.horizon.visible(direction, 0.0) {
                continue;
            }
            let point = camera.project(direction);
            if point.visible {
                astro::text::draw_moving_text(
                    &painter,
                    Pos2::new(
                        rect.left() + point.x as f32 / ppp,
                        rect.top() + point.y as f32 / ppp,
                    ),
                    Color32::from_rgb(150, 180, 210),
                    font.clone(),
                    tr.translate(&figure.name),
                );
            }
        }
    }
    if let Some(selected) = selected {
        if !view.ground
            || view
                .horizon
                .visible(selected.observed, selected.angular_radius)
        {
            let point = camera.project(selected.observed);
            if point.visible {
                painter.circle_stroke(
                    Pos2::new(
                        rect.left() + point.x as f32 / ppp,
                        rect.top() + point.y as f32 / ppp,
                    ),
                    9.0,
                    egui::Stroke::new(1.0_f32, Color32::YELLOW),
                );
            }
        }
    }
}

fn draw_grid(painter: &egui::Painter, camera: &Camera, rect: Rect, ppp: f32) {
    let curve = |directions: Vec<Vec3>| {
        let mut prior: Option<Pos2> = None;
        for d in directions {
            let p = camera.project(d);
            if p.visible {
                let next = Pos2::new(rect.min.x + p.x as f32 / ppp, rect.min.y + p.y as f32 / ppp);
                if let Some(prior) = prior {
                    if prior.distance(next) < rect.width() * 0.1 {
                        painter.line_segment(
                            [prior, next],
                            egui::Stroke::new(
                                0.5_f32,
                                Color32::from_rgba_unmultiplied(80, 130, 160, 120),
                            ),
                        );
                    }
                }
                prior = Some(next);
            } else {
                prior = None;
            }
        }
    };
    for elevation in (-60..=60).step_by(15) {
        curve(
            (0..=360)
                .step_by(2)
                .map(|az| Vec3 {
                    x: (az as f64 * RAD).sin() * (elevation as f64 * RAD).cos(),
                    y: (az as f64 * RAD).cos() * (elevation as f64 * RAD).cos(),
                    z: (elevation as f64 * RAD).sin(),
                })
                .collect(),
        );
    }
    for az in (0..360).step_by(30) {
        curve(
            (-90..=90)
                .step_by(2)
                .map(|el| Vec3 {
                    x: (az as f64 * RAD).sin() * (el as f64 * RAD).cos(),
                    y: (az as f64 * RAD).cos() * (el as f64 * RAD).cos(),
                    z: (el as f64 * RAD).sin(),
                })
                .collect(),
        );
    }
}
fn run() -> Result<()> {
    let Some(options) = options()? else {
        return Ok(());
    };
    // Set packaged driver discovery before starting any workers.
    if std::env::var_os("VK_DRIVER_FILES").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let driver = parent.join("../Resources/vulkan/icd.d/MoltenVK_icd.json");
                if driver.exists() {
                    unsafe {
                        std::env::set_var("VK_DRIVER_FILES", driver);
                    }
                }
            }
        }
    }
    let engine = Arc::new(SkyEngine::new(&options.data)?);
    let mut renderer = Renderer::new(&options.data, options.validation)?;
    eprintln!("GPU: {}", renderer.gpu.name);
    if let Some(r) = options.sequence.clone() {
        export_sequence(
            &engine,
            &mut renderer,
            &options.scene,
            r,
            options.width,
            options.height,
            None,
            None,
        )?;
        ensure!(renderer.gpu.errors() == 0, "Vulkan validation errors");
        return Ok(());
    }
    if options.headless {
        let mut s = options.scene.clone();
        let mut result = None;
        for frame in 0..options.frames.max(1) {
            if frame > 0 && options.speed != 0. {
                s = scene_at(&s, options.speed / 60.)?;
            }
            let (image, sky) = make_image(
                &engine,
                &mut renderer,
                &s,
                options.width,
                options.height,
                1. / 60.,
            )?;
            result = Some((image, sky));
        }
        if let Some(path) = &options.screenshot {
            let (image, sky) = result.unwrap();
            save_image(path, &image, &s, &sky, renderer.effective_exposure)?;
        }
        println!("Validation errors: {}", renderer.gpu.errors());
        ensure!(renderer.gpu.errors() == 0, "Vulkan validation errors");
        return Ok(());
    }
    let validation_gpu = renderer.gpu.clone();
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([options.width as f32, options.height as f32])
            .with_title("Astra"),
        ..Default::default()
    };
    eframe::run_native(
        "Astra",
        native,
        Box::new(move |cc| Ok(Box::new(App::new(cc, options, engine, renderer)?))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    ensure!(
        validation_gpu.errors() == 0,
        "Vulkan validation errors: {}",
        validation_gpu.errors()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Astra: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod sky_overlay_tests {
    use super::*;
    use astro::catalog::{Catalog, Constellation, Star};

    #[test]
    fn comparison_overlays_use_each_epoch_and_clip_each_viewport() {
        let view = Scenario {
            atmosphere: false,
            ground: false,
            elevation: 0.0,
            grid: true,
            labels: true,
            constellations: true,
            constellation_labels: true,
            ..Scenario::default()
        };
        let camera = view.camera(200.0, 200.0);
        let ray = camera.forward();
        let mut catalog = Catalog::default();
        catalog.stars = vec![
            Star {
                hip: 1,
                ..Star::default()
            },
            Star {
                hip: 2,
                ..Star::default()
            },
        ];
        catalog.constellations = vec![Constellation {
            id: "Ori".into(),
            name: "Orion".into(),
            segments: vec![[1, 2]],
        }];
        let left_star = Object {
            id: 1,
            catalog_index: 0,
            icrs: ray,
            ..Object::default()
        };
        let right_matrix = enu_basis(0.0, 0.0);
        let right_star = Object {
            id: 2,
            catalog_index: 1,
            icrs: right_matrix.transpose() * ray,
            ..Object::default()
        };
        let left = SkySnapshot {
            scenario: view.clone(),
            bodies: vec![Object {
                body: 10,
                observed: ray,
                ..Object::default()
            }],
            stars: vec![left_star],
            label_stars: vec![0],
            guide_stars: [(1, left_star)].into_iter().collect(),
            ..SkySnapshot::default()
        };
        let right = SkySnapshot {
            scenario: view.clone(),
            celestial_to_enu: right_matrix,
            bodies: vec![Object {
                body: 301,
                observed: ray,
                ..Object::default()
            }],
            stars: vec![right_star],
            label_stars: vec![0],
            guide_stars: [(2, right_star)].into_iter().collect(),
            ..SkySnapshot::default()
        };
        let left_rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 200.0));
        let right_rect = Rect::from_min_max(Pos2::new(200.0, 0.0), Pos2::new(400.0, 200.0));
        let context = egui::Context::default();
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_max(left_rect.min, right_rect.max)),
                ..egui::RawInput::default()
            },
            |context| {
                let painter = context.layer_painter(egui::LayerId::background());
                draw_sky_overlay(
                    &painter,
                    &left,
                    &left,
                    &view,
                    &camera,
                    left_rect,
                    1.0,
                    &catalog,
                    Language::English,
                    None,
                );
                draw_sky_overlay(
                    &painter,
                    &right,
                    &right,
                    &view,
                    &camera,
                    right_rect,
                    1.0,
                    &catalog,
                    Language::English,
                    None,
                );
            },
        );
        let text = |clip: Rect| {
            output
                .shapes
                .iter()
                .filter(|s| s.clip_rect == clip)
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(shape) => {
                        Some((shape.galley.job.text.clone(), shape.pos))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let left_text = text(left_rect);
        let right_text = text(right_rect);
        assert!(left_text.iter().any(|(s, _)| s == "Sun"));
        assert!(right_text.iter().any(|(s, _)| s == "Moon"));
        assert!(
            left_text
                .iter()
                .any(|(s, _)| s == &catalog.name(&catalog.stars[0]))
        );
        assert!(
            right_text
                .iter()
                .any(|(s, _)| s == &catalog.name(&catalog.stars[1]))
        );
        assert!(
            left_text.iter().any(|(s, _)| s == "Orion")
                && right_text.iter().any(|(s, _)| s == "Orion")
        );
        for (text, left_pos) in &left_text {
            if text == "Orion" {
                let right_pos = right_text.iter().find(|(s, _)| s == text).unwrap().1;
                assert!((right_pos.x - left_pos.x - 200.0).abs() < 1e-4);
            }
        }
        assert!(
            output
                .shapes
                .iter()
                .all(|s| s.clip_rect == left_rect || s.clip_rect == right_rect)
        );
        for clip in [left_rect, right_rect] {
            assert!(output.shapes.iter().any(|s| s.clip_rect == clip
                && matches!(s.shape, egui::epaint::Shape::LineSegment { .. })));
        }
    }

    #[test]
    fn picking_respects_disc_extent_ground_and_fisheye_black_corners() {
        let mut view = Scenario {
            atmosphere: false,
            ground: false,
            elevation: 0.0,
            ..Scenario::default()
        };
        let camera = view.camera(400.0, 400.0);
        let ray = camera.forward();
        let frame = SkySnapshot {
            scenario: view.clone(),
            bodies: vec![Object {
                body: 301,
                id: 301,
                observed: ray,
                angular_radius: 0.1,
                ..Object::default()
            }],
            ..SkySnapshot::default()
        };
        let radius = 0.1 * camera.pixels_per_radian();
        let picked = pick_sky_object(
            &frame,
            &frame,
            &view,
            &camera,
            200.0 + radius * 0.9,
            200.0,
            2.0,
        )
        .unwrap();
        assert_eq!(picked.body, 301);
        assert!(
            pick_sky_object(
                &frame,
                &frame,
                &view,
                &camera,
                200.0 + radius + 4.0,
                200.0,
                2.0
            )
            .is_none()
        );
        view.ground = true;
        view.horizon.degrees.fill(20.0);
        assert!(pick_sky_object(&frame, &frame, &view, &camera, 200.0, 200.0, 18.0).is_none());
        view.ground = false;
        let fisheye = Camera {
            projection: ProjectionKind::Fisheye,
            ..camera
        };
        assert!(pick_sky_object(&frame, &frame, &view, &fisheye, 1.0, 1.0, 500.0).is_none());
    }
}
