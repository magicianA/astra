use crate::{
    events::{
        EventKind, EventSearch, ObservationPlan, SkyEvent, Target, find_events, observing_plan,
        scene_at, target_object,
    },
    horizon::{Horizon, load_horizon},
    i18n::{Language, Translator},
    math::{RAD, Vec3, norm, unit, wrap},
    scenario::{Scenario, save_scenario},
    sequence::{SequenceRequest, validate_sequence},
    sky::{Object, Scope, SkyEngine, SkySnapshot, body_name},
    time::format_date,
};
use anyhow::{Context, Result, bail};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

const BODIES: [i32; 9] = [10, 301, 199, 299, 4, 5, 6, 7, 8];
#[derive(Clone)]
pub struct ExploreResult {
    pub scene: Scenario,
    pub search: EventSearch,
    pub plan: Option<ObservationPlan>,
    pub events: Vec<SkyEvent>,
}
struct Job {
    receiver: Receiver<Result<ExploreResult>>,
    cancel: Arc<AtomicBool>,
}

pub struct Exploration {
    pub language: Language,
    pub tab: usize,
    pub figure: usize,
    pub target_body: usize,
    pub event_kind: usize,
    pub first_body: usize,
    pub second_body: usize,
    pub event_days: f64,
    pub separation_degrees: f64,
    pub use_selected: bool,
    pub use_selected_event: bool,
    pub result: Option<ExploreResult>,
    pub sequence: SequenceRequest,
    pub pending_recording: Option<SequenceRequest>,
    pub recording: bool,
    pub cancel_recording: bool,
    pub completed_frames: i32,
    /// The main app consumes this to select/track the jumped-to target and optionally play it.
    pub pending_selection: Option<(Object, bool)>,
    pub notice: Option<String>,
    pub horizon_path: String,
    pub output_path: String,
    job: Option<Job>,
    picker: Option<Receiver<Result<(bool, PathBuf)>>>,
}
impl Default for Exploration {
    fn default() -> Self {
        Self {
            language: Language::Chinese,
            tab: 0,
            figure: 0,
            target_body: 1,
            event_kind: 0,
            first_body: 1,
            second_body: 0,
            event_days: 35.0,
            separation_degrees: 3.0,
            use_selected: false,
            use_selected_event: false,
            result: None,
            sequence: SequenceRequest::default(),
            pending_recording: None,
            recording: false,
            cancel_recording: false,
            completed_frames: 0,
            pending_selection: None,
            notice: None,
            horizon_path: String::new(),
            output_path: String::new(),
            job: None,
            picker: None,
        }
    }
}
impl Drop for Exploration {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

pub fn selected_target(
    selected: Option<Object>,
    fallback: usize,
    use_selected: bool,
) -> Result<Target> {
    if use_selected {
        let object = selected.context("Select an object first")?;
        if object.body != 0 {
            Ok(Target {
                body: object.body,
                star: None,
            })
        } else if object.catalog_index != u32::MAX {
            Ok(Target {
                body: 0,
                star: Some(object.catalog_index),
            })
        } else {
            bail!("Select an object first")
        }
    } else {
        Ok(Target {
            body: BODIES[fallback.min(BODIES.len() - 1)],
            star: None,
        })
    }
}
fn clock_text(initial: &Scenario, seconds: f64) -> Result<String> {
    Ok(format_date(&scene_at(initial, seconds)?.date))
}
fn jump(
    scene: &mut Scenario,
    engine: &SkyEngine,
    base: &Scenario,
    seconds: f64,
    target: Target,
) -> Result<Object> {
    *scene = scene_at(base, seconds)?;
    let sky = engine.compute(
        scene,
        0,
        None,
        if target.star.is_some() {
            Scope::FullSky
        } else {
            Scope::SolarSystem
        },
        target.star,
    )?;
    let object = target_object(&sky, target)?;
    scene.azimuth = object.azimuth() / RAD;
    scene.elevation = object.altitude() / RAD;
    Ok(object)
}
fn body_combo(ui: &mut egui::Ui, id: &str, index: &mut usize, tr: Translator) {
    *index = (*index).min(BODIES.len() - 1);
    egui::ComboBox::from_id_salt(id)
        .selected_text(tr.translate(&body_name(BODIES[*index])))
        .show_ui(ui, |ui| {
            for (i, body) in BODIES.iter().enumerate() {
                ui.selectable_value(index, i, tr.translate(&body_name(*body)));
            }
        });
}
pub fn export_observation_plan(
    plan: &ObservationPlan,
    path: impl AsRef<Path>,
    data_id: &str,
) -> Result<()> {
    let path = path.as_ref();
    let mut csv =
        String::from("time,altitude_deg,sun_altitude_deg,moon_separation_deg,sky_cd_m2,suitable\n");
    for p in &plan.samples {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            clock_text(&plan.initial, p.seconds)?,
            p.altitude / RAD,
            p.sun_altitude / RAD,
            p.moon_separation / RAD,
            p.sky_luminance,
            u8::from(p.suitable)
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, csv).context("Cannot write observation plan")?;
    save_scenario(
        &plan.initial,
        path.with_extension("json"),
        data_id,
        None,
        0.0,
    )
}
pub fn export_event_table(
    result: &ExploreResult,
    path: impl AsRef<Path>,
    data_id: &str,
) -> Result<()> {
    let path = path.as_ref();
    let mut csv = String::from(
        "type,peak,first_contact,last_contact,separation_deg,obscuration,locally_visible\n",
    );
    for event in &result.events {
        let first = event
            .start
            .map(|s| clock_text(&result.scene, s))
            .transpose()?
            .unwrap_or_default();
        let last = event
            .end
            .map(|s| clock_text(&result.scene, s))
            .transpose()?
            .unwrap_or_default();
        let kind = format!("\"{}\"", event.r#type.replace('"', "\"\""));
        csv.push_str(&format!(
            "{kind},{},{first},{last},{},{},{}\n",
            clock_text(&result.scene, event.peak)?,
            event.separation / RAD,
            event.obscuration,
            u8::from(event.visible)
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, csv)?;
    save_scenario(
        &result.scene,
        path.with_extension("json"),
        data_id,
        None,
        0.0,
    )?;
    std::fs::write(
        path.with_extension("events.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"search":result.search,"events":result.events}),
        )?,
    )?;
    Ok(())
}
fn plot_plan(ui: &mut egui::Ui, plan: &ObservationPlan, tr: Translator) -> Result<Option<f64>> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(40.0), 190.0),
        Sense::click(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, Color32::from_rgb(12, 19, 30));
    let xy = |t: f64, alt: f64| {
        Pos2::new(
            rect.left() + (t / 86400.0) as f32 * rect.width(),
            rect.bottom() - ((alt / RAD).clamp(-30.0, 90.0) as f32 + 30.0) / 120.0 * rect.height(),
        )
    };
    for window in &plan.windows {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(xy(window.start, 0.0).x, rect.top()),
                Pos2::new(xy(window.end, 0.0).x, rect.bottom()),
            ),
            0.0,
            Color32::from_rgba_unmultiplied(43, 116, 83, 45),
        );
    }
    for altitude in [-18.0, 0.0, 30.0, 60.0] {
        painter.line_segment(
            [xy(0.0, altitude * RAD), xy(86400.0, altitude * RAD)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 90, 100, 70)),
        );
        painter.text(
            xy(0.0, altitude * RAD),
            egui::Align2::LEFT_TOP,
            format!("{altitude:.0}"),
            egui::FontId::proportional(10.0),
            Color32::GRAY,
        );
    }
    for pair in plan.samples.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        painter.line_segment(
            [xy(a.seconds, a.altitude), xy(b.seconds, b.altitude)],
            Stroke::new(2.0_f32, Color32::from_rgb(118, 215, 217)),
        );
        painter.line_segment(
            [xy(a.seconds, a.sun_altitude), xy(b.seconds, b.sun_altitude)],
            Stroke::new(1.0_f32, Color32::from_rgb(226, 173, 103)),
        );
        painter.line_segment(
            [
                xy(a.seconds, a.moon_altitude),
                xy(b.seconds, b.moon_altitude),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(162, 161, 213)),
        );
    }
    for hour in [0, 6, 12, 18, 24] {
        let date = scene_at(&plan.initial, hour as f64 * 3600.0)?.date;
        let x = xy(hour as f64 * 3600.0, 0.0).x;
        painter.text(
            Pos2::new(
                x.clamp(rect.left() + 15.0, rect.right() - 15.0),
                rect.bottom(),
            ),
            egui::Align2::CENTER_BOTTOM,
            format!("{:02}:{:02}", date.hour, date.minute),
            egui::FontId::proportional(10.0),
            Color32::GRAY,
        );
    }
    let mut picked = None;
    if response.hovered() && !plan.samples.is_empty() {
        if let Some(pointer) = response.hover_pos() {
            let i = (((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0)
                * (plan.samples.len() - 1) as f32) as usize;
            let p = plan.samples[i];
            let tooltip = format!(
                "{}\n{} {:.1}°\n{} {:.1}°",
                clock_text(&plan.initial, p.seconds)?,
                tr.translate("目标高度"),
                p.altitude / RAD,
                tr.translate("距月球"),
                p.moon_separation / RAD
            );
            if response.clicked() {
                picked = Some(p.seconds);
            }
            response.on_hover_text(tooltip);
        }
    }
    ui.label(tr.translate("青色：目标  金色：太阳  紫色：月球；点击曲线前往"));
    Ok(picked)
}

impl Exploration {
    fn start_job(
        &mut self,
        ui: &egui::Ui,
        operation: impl FnOnce(Arc<AtomicBool>) -> Result<ExploreResult> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let context = ui.ctx().clone();
        std::thread::spawn(move || {
            let result = operation(token);
            let _ = sender.send(result);
            context.request_repaint();
        });
        self.job = Some(Job { receiver, cancel });
    }
    fn browse(&mut self, ui: &egui::Ui, folder: bool) {
        if self.picker.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let context = ui.ctx().clone();
        std::thread::spawn(move || {
            let result = pick_path(folder).map(|path| (folder, path));
            let _ = sender.send(result);
            context.request_repaint();
        });
        self.picker = Some(receiver);
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        scene: &mut Scenario,
        engine: Arc<SkyEngine>,
        sky: Option<&SkySnapshot>,
        selected: Option<Object>,
        _data: &Path,
        user: &Path,
    ) -> Result<bool> {
        let tr = Translator {
            language: self.language,
        };
        let mut changed = false;
        if let Some(job) = &self.job {
            match job.receiver.try_recv() {
                Ok(result) => {
                    self.job = None;
                    self.result = Some(result?);
                }
                Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    bail!("Background calculation ended unexpectedly");
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(receiver) = &self.picker {
            match receiver.try_recv() {
                Ok(result) => {
                    self.picker = None;
                    match result {
                        Ok((folder, path)) => {
                            if !path.as_os_str().is_empty() {
                                if folder {
                                    self.output_path = path.display().to_string();
                                } else {
                                    self.horizon_path = path.display().to_string();
                                }
                            }
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
                Err(TryRecvError::Disconnected) => self.picker = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        let tabs = ["星座与年代", "观测计划", "地景", "录制与导出", "天象搜索"];
        self.tab = self.tab.min(tabs.len() - 1);
        egui::ComboBox::from_id_salt("explore-section")
            .selected_text(tr.translate(tabs[self.tab]))
            .show_ui(ui, |ui| {
                for (i, tab) in tabs.iter().enumerate() {
                    ui.selectable_value(&mut self.tab, i, tr.translate(tab));
                }
            });
        ui.separator();
        match self.tab {
            0 => {
                changed |= ui
                    .checkbox(&mut scene.constellations, tr.translate("星座连线"))
                    .changed();
                changed |= ui
                    .checkbox(&mut scene.constellation_labels, tr.translate("星座名称"))
                    .changed();
                let figures = &engine.catalog.constellations;
                if !figures.is_empty() {
                    self.figure = self.figure.min(figures.len() - 1);
                    egui::ComboBox::from_id_salt("constellation")
                        .selected_text(tr.translate(&figures[self.figure].name))
                        .show_ui(ui, |ui| {
                            for (i, figure) in figures.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.figure,
                                    i,
                                    tr.translate(&figure.name),
                                );
                            }
                        });
                    if ui.button(tr.translate("定位星座")).clicked() {
                        scene.constellations = true;
                        let computed = engine.compute(scene, 0, None, Scope::FullSky, None)?;
                        let mut centre = Vec3::default();
                        for segment in &figures[self.figure].segments {
                            for hip in segment {
                                if let Some(star) = computed.guide_stars.get(hip) {
                                    centre += star.observed;
                                }
                            }
                        }
                        if norm(centre) > 1e-8 {
                            centre = unit(centre);
                            scene.azimuth = wrap(centre.x.atan2(centre.y)) / RAD;
                            scene.elevation = centre.z.asin() / RAD;
                        }
                        scene.fov = 60.0;
                        changed = true;
                    }
                }
                ui.label(tr.translate("连线跟随恒星运动；现代星座图形不代表古代星官。"));
                ui.separator();
                changed |= ui
                    .checkbox(&mut scene.compare, tr.translate("双年代并排对照"))
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut scene.comparison_year, -3000..=6999)
                            .text(tr.translate("对照年份")),
                    )
                    .changed();
                ui.label(
                    tr.translate("两侧共用地点、月日、时刻和镜头。拖动同步环顾，在左侧选取天体。"),
                );
                ui.separator();
                changed |= ui
                    .checkbox(&mut scene.moon_surface, tr.translate("月面地形与阴影"))
                    .changed();
                changed |= ui
                    .checkbox(&mut scene.earthshine, tr.translate("地球照"))
                    .changed();
                if ui.button(tr.translate("月面细节曝光")).clicked() {
                    scene.auto_exposure = false;
                    scene.adaptive_exposure = false;
                    scene.exposure = 2.0_f64.powi(-18);
                    let base = scene.clone();
                    let object = jump(scene, &engine, &base, 0.0, Target::default())?;
                    self.pending_selection = Some((object, false));
                    scene.fov = 1.2;
                    changed = true;
                }
                if let Some(sky) = sky {
                    ui.label(tr.translate(if sky.lunar.precise_orientation {
                        "月面姿态：DE440 数值天平动"
                    } else {
                        "月面姿态：IAU 近似，远年代未经精度验证"
                    }));
                }
            }
            1 => {
                body_combo(ui, "plan-target", &mut self.target_body, tr);
                ui.checkbox(&mut self.use_selected, tr.translate("使用当前选中天体"));
                if ui
                    .add_enabled(
                        self.job.is_none(),
                        egui::Button::new(tr.translate("计算未来 24 小时")),
                    )
                    .clicked()
                {
                    let target = selected_target(selected, self.target_body, self.use_selected)?;
                    let initial = scene.clone();
                    let engine = engine.clone();
                    self.start_job(ui, move |cancel| {
                        let check = || cancel.load(Ordering::Relaxed);
                        let plan = observing_plan(&engine, &initial, target, Some(&check))?;
                        Ok(ExploreResult {
                            scene: initial,
                            search: EventSearch::default(),
                            plan: Some(plan),
                            events: Vec::new(),
                        })
                    });
                }
                if let Some(plan) = self.result.as_ref().and_then(|r| r.plan.as_ref()) {
                    ui.label(format!(
                        "{} · {}",
                        tr.translate(&plan.initial.location_name),
                        format_date(&plan.initial.date)
                    ));
                    if let Some(seconds) = plot_plan(ui, plan, tr)? {
                        self.pending_selection = Some((
                            jump(scene, &engine, &plan.initial, seconds, plan.target)?,
                            false,
                        ));
                        changed = true;
                    }
                    ui.label(format!(
                        "{}: {} ({:.1}°)",
                        tr.translate("最高位置"),
                        clock_text(&plan.initial, plan.transit)?,
                        plan.transit_altitude / RAD
                    ));
                    for seconds in &plan.rises {
                        ui.label(format!(
                            "{}: {}",
                            tr.translate("升起"),
                            clock_text(&plan.initial, *seconds)?
                        ));
                    }
                    for seconds in &plan.sets {
                        ui.label(format!(
                            "{}: {}",
                            tr.translate("落下"),
                            clock_text(&plan.initial, *seconds)?
                        ));
                    }
                    if plan.rises.is_empty() && plan.sets.is_empty() {
                        ui.label(tr.translate("本区间没有升落事件。"));
                    }
                    if plan.has_recommendation {
                        if ui.button(tr.translate("前往推荐时刻")).clicked() {
                            self.pending_selection = Some((
                                jump(scene, &engine, &plan.initial, plan.best_time, plan.target)?,
                                false,
                            ));
                            changed = true;
                        }
                        for interval in &plan.windows {
                            ui.label(format!(
                                "{} — {}",
                                clock_text(&plan.initial, interval.start)?,
                                clock_text(&plan.initial, interval.end)?
                            ));
                        }
                    } else {
                        ui.label(tr.translate("本区间没有满足条件的观测窗口。"));
                    }
                    if ui.button(tr.translate("导出观测表")).clicked() {
                        let path = user.join("observing-plan.csv");
                        export_observation_plan(plan, &path, &engine.catalog.data_id)?;
                        self.notice = Some(path.display().to_string());
                    }
                }
                ui.label(tr.translate(
                    "按高度、夜色、月光和地平线筛选；不包含天气预报。绿色窗口按 5 分钟采样。",
                ));
            }
            2 => {
                ui.label(tr.translate(
                    "导入 CSV 的方位角、高度角两列，或 JSON points 数组。北方为 0°，向东增加。",
                ));
                ui.text_edit_singleline(&mut self.horizon_path);
                ui.horizontal(|ui| {
                    if ui.button(tr.translate("选择轮廓文件")).clicked() {
                        self.browse(ui, false);
                    }
                });
                if ui.button(tr.translate("应用地平线")).clicked() {
                    scene.horizon = load_horizon(&self.horizon_path)?;
                    scene.ground = true;
                    changed = true;
                }
                ui.label(format!(
                    "{}: {}",
                    tr.translate("当前地景"),
                    if scene.horizon.name.is_empty() {
                        tr.translate("平坦地平线")
                    } else {
                        &scene.horizon.name
                    }
                ));
                if ui.button(tr.translate("恢复平坦地平线")).clicked() {
                    scene.horizon = Horizon::default();
                    changed = true;
                }
                ui.label(
                    tr.translate("轮廓会遮挡天体，并参与升落和观测计划计算；保存场景时一并保存。"),
                );
            }
            3 => {
                ui.label(tr.translate("导出目录"));
                ui.text_edit_singleline(&mut self.output_path);
                if ui.button(tr.translate("选择导出位置")).clicked() {
                    self.browse(ui, true);
                }
                ui.horizontal(|ui| {
                    ui.label(tr.translate("帧数"));
                    ui.add(egui::DragValue::new(&mut self.sequence.frames).range(2..=100000));
                });
                ui.horizontal(|ui| {
                    ui.label(tr.translate("每帧推进秒数"));
                    ui.add(egui::DragValue::new(&mut self.sequence.step_seconds).speed(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label(tr.translate("视频帧率"));
                    ui.add(egui::DragValue::new(&mut self.sequence.fps).range(1..=120));
                });
                ui.checkbox(&mut self.sequence.video, tr.translate("同时导出 MP4"));
                ui.checkbox(&mut self.sequence.trails, tr.translate("星轨叠加图"));
                ui.checkbox(&mut self.sequence.lock_exposure, tr.translate("锁定曝光"));
                ui.checkbox(&mut self.sequence.camera_path, tr.translate("镜头运动"));
                if ui.button(tr.translate("将当前视角设为终点")).clicked() {
                    self.sequence.camera_end = scene.clone();
                }
                ui.label(
                    tr.translate(
                        "开始时的视角作为起点；星轨使用逐帧变亮叠加，不等同于物理长曝光。",
                    ),
                );
                if ui
                    .add_enabled(!self.recording, egui::Button::new(tr.translate("开始导出")))
                    .clicked()
                {
                    self.sequence.directory = if self.output_path.is_empty() {
                        user.join("exports")
                    } else {
                        PathBuf::from(&self.output_path)
                    };
                    validate_sequence(&self.sequence)?;
                    self.pending_recording = Some(self.sequence.clone());
                }
                if self.recording {
                    ui.label(format!(
                        "{} {} / {}",
                        tr.translate("正在导出"),
                        self.completed_frames,
                        self.sequence.frames
                    ));
                    if ui.button(tr.translate("取消导出")).clicked() {
                        self.cancel_recording = true;
                    }
                }
                ui.label(tr.translate(
                    "每帧等待精确计算，保存 PNG 和场景。MP4 使用 FFmpeg；取消后保留已完成帧。",
                ));
            }
            _ => {
                let kinds = ["日食", "月食", "天体接近", "掩星"];
                self.event_kind = self.event_kind.min(3);
                egui::ComboBox::from_id_salt("event-kind")
                    .selected_text(tr.translate(kinds[self.event_kind]))
                    .show_ui(ui, |ui| {
                        for (i, kind) in kinds.iter().enumerate() {
                            ui.selectable_value(&mut self.event_kind, i, tr.translate(kind));
                        }
                    });
                if self.event_kind >= 2 {
                    body_combo(ui, "event-first", &mut self.first_body, tr);
                    body_combo(ui, "event-second", &mut self.second_body, tr);
                    ui.checkbox(
                        &mut self.use_selected_event,
                        tr.translate("第二目标使用选中天体"),
                    );
                }
                ui.horizontal(|ui| {
                    ui.label(tr.translate("搜索天数"));
                    ui.add(
                        egui::DragValue::new(&mut self.event_days)
                            .speed(1.0)
                            .range(0.01..=370.0),
                    );
                });
                if self.event_kind == 2 {
                    ui.horizontal(|ui| {
                        ui.label(tr.translate("最大角距（度）"));
                        ui.add(
                            egui::DragValue::new(&mut self.separation_degrees)
                                .speed(0.1)
                                .range(0.001..=180.0),
                        );
                    });
                }
                if ui
                    .add_enabled(
                        self.job.is_none(),
                        egui::Button::new(tr.translate("搜索天象")),
                    )
                    .clicked()
                {
                    let request = EventSearch {
                        kind: match self.event_kind {
                            0 => EventKind::SolarEclipse,
                            1 => EventKind::LunarEclipse,
                            2 => EventKind::CloseApproach,
                            _ => EventKind::Occultation,
                        },
                        first: Target {
                            body: BODIES[self.first_body],
                            star: None,
                        },
                        second: selected_target(
                            selected,
                            self.second_body,
                            self.event_kind >= 2 && self.use_selected_event,
                        )?,
                        days: self.event_days,
                        maximum_separation: self.separation_degrees * RAD,
                    };
                    let initial = scene.clone();
                    let engine = engine.clone();
                    self.start_job(ui, move |cancel| {
                        let check = || cancel.load(Ordering::Relaxed);
                        let events = find_events(&engine, &initial, &request, Some(&check))?;
                        Ok(ExploreResult {
                            scene: initial,
                            search: request,
                            plan: None,
                            events,
                        })
                    });
                }
                if let Some(result) = self.result.as_ref().filter(|r| r.plan.is_none()) {
                    ui.label(format!(
                        "{} · {}",
                        tr.translate(&result.scene.location_name),
                        format_date(&result.scene.date)
                    ));
                    if result.events.is_empty() {
                        ui.label(tr.translate("此区间未找到符合条件的天象。"));
                    }
                    for (i, event) in result.events.iter().enumerate() {
                        ui.push_id(i, |ui| -> Result<()> {
                            ui.separator();
                            ui.label(format!(
                                "{} · {}",
                                tr.translate(&event.r#type),
                                tr.translate(if event.visible {
                                    "当地可见"
                                } else {
                                    "地平线以下"
                                })
                            ));
                            ui.label(clock_text(&result.scene, event.peak)?);
                            ui.label(format!(
                                "{} {:.4}°",
                                tr.translate("最小角距"),
                                event.separation / RAD
                            ));
                            if let Some(time) = event.start {
                                ui.label(format!(
                                    "{}: {}",
                                    tr.translate("开始接触"),
                                    clock_text(&result.scene, time)?
                                ));
                            }
                            if let Some(time) = event.end {
                                ui.label(format!(
                                    "{}: {}",
                                    tr.translate("结束接触"),
                                    clock_text(&result.scene, time)?
                                ));
                            }
                            let target = match event.kind {
                                EventKind::SolarEclipse => Target {
                                    body: 10,
                                    star: None,
                                },
                                EventKind::LunarEclipse => Target::default(),
                                _ => result.search.first,
                            };
                            if ui.button(tr.translate("查看峰值")).clicked() {
                                self.pending_selection = Some((
                                    jump(scene, &engine, &result.scene, event.peak, target)?,
                                    false,
                                ));
                                changed = true;
                            }
                            if ui.button(tr.translate("播放过程")).clicked() {
                                self.pending_selection = Some((
                                    jump(
                                        scene,
                                        &engine,
                                        &result.scene,
                                        event.start.unwrap_or(event.peak - 3600.0) - 300.0,
                                        target,
                                    )?,
                                    true,
                                ));
                                changed = true;
                            }
                            Ok(())
                        })
                        .inner?;
                    }
                    if ui
                        .button(if self.language == Language::Chinese {
                            "导出天象表"
                        } else {
                            "Export event table"
                        })
                        .clicked()
                    {
                        let path = user.join("events.csv");
                        export_event_table(result, &path, &engine.catalog.data_id)?;
                        self.notice = Some(path.display().to_string());
                    }
                }
                ui.label(tr.translate(
                    "接触时刻采用球形天体模型；远年代受 ΔT 影响。月食红光及日食天空变暗为近似。",
                ));
            }
        }
        if let Some(job) = &self.job {
            ui.separator();
            ui.label(tr.translate("后台计算中…"));
            if ui.button(tr.translate("取消计算")).clicked() {
                job.cancel.store(true, Ordering::Relaxed);
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        Ok(changed)
    }
}

/// Native picker runs on a worker so modal platform dialogs never stall rendering.
fn pick_path(folder: bool) -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            if folder {
                "POSIX path of (choose folder)"
            } else {
                "POSIX path of (choose file)"
            },
        ])
        .output()?;
    #[cfg(target_os = "linux")]
    let output = {
        let mut command = std::process::Command::new("zenity");
        command.arg("--file-selection");
        if folder {
            command.arg("--directory");
        }
        command.output()?
    };
    #[cfg(target_os="windows")]
    let output=std::process::Command::new("powershell").args(["-NoProfile","-STA","-Command",if folder{"Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.FolderBrowserDialog; if($d.ShowDialog() -eq 'OK'){$d.SelectedPath}"}else{"Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.OpenFileDialog; if($d.ShowDialog() -eq 'OK'){$d.FileName}"}]).output()?;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    bail!("Enter the file or directory path manually");
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        if !output.status.success() {
            return Ok(PathBuf::new());
        }
        Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_target_retains_catalog_index_not_display_id() {
        let star = Object {
            id: 12345,
            catalog_index: 27,
            ..Object::default()
        };
        assert_eq!(selected_target(Some(star), 0, true).unwrap().star, Some(27));
        let moon = Object {
            body: 301,
            ..Object::default()
        };
        assert_eq!(selected_target(Some(moon), 0, true).unwrap().body, 301);
        assert!(selected_target(None, 0, true).is_err());
    }
}
