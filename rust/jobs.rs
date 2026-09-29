//! Background astronomy, navigation, and exact fixed-step exports for the desktop app.
use crate::{
    recording::Recording,
    render_scene::{RenderScene, render_scene},
    renderer::Renderer,
    scenario::{Scenario, save_scenario},
    sequence::{SequenceRequest, sequence_scene},
    sky::{Scope, SkyEngine, SkySnapshot},
    time::{TimeScale, valid_date},
};
use anyhow::{Result, bail, ensure};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct SkyPair {
    pub primary: SkySnapshot,
    pub comparison: Option<SkySnapshot>,
}

struct LatestControl<J> {
    sender: Option<Sender<(u64, J)>>,
    latest: Arc<AtomicU64>,
    /// Serialize publishing with requests so superseded results cannot enter the
    /// output queue after a newer request has drained it.
    publish: Arc<Mutex<()>>,
    handle: Option<JoinHandle<()>>,
}
impl<J> LatestControl<J> {
    fn request<T>(&self, job: J, receiver: &Receiver<Result<T>>) {
        let _gate = self.publish.lock().unwrap_or_else(|e| e.into_inner());
        let generation = self.latest.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        while receiver.try_recv().is_ok() {}
        if let Some(sender) = &self.sender {
            let _ = sender.send((generation, job));
        }
    }
    fn cancel<T>(&self, receiver: &Receiver<Result<T>>) {
        let _gate = self.publish.lock().unwrap_or_else(|e| e.into_inner());
        self.latest.fetch_add(1, Ordering::SeqCst);
        while receiver.try_recv().is_ok() {}
    }
}
impl<J> Drop for LatestControl<J> {
    fn drop(&mut self) {
        {
            let _gate = self.publish.lock().unwrap_or_else(|e| e.into_inner());
            self.latest.fetch_add(1, Ordering::SeqCst);
            self.sender.take();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
fn latest_worker<J, T>(
    operation: impl Fn(J, u64, &AtomicU64) -> Result<T> + Send + 'static,
) -> (LatestControl<J>, Receiver<Result<T>>)
where
    J: Send + 'static,
    T: Send + 'static,
{
    let (sender, input) = mpsc::channel::<(u64, J)>();
    let (output, receiver) = mpsc::channel();
    let latest = Arc::new(AtomicU64::new(0));
    let publish = Arc::new(Mutex::new(()));
    let generation = latest.clone();
    let gate = publish.clone();
    let handle = thread::spawn(move || {
        while let Ok(mut job) = input.recv() {
            while let Ok(newest) = input.try_recv() {
                job = newest;
            }
            if generation.load(Ordering::SeqCst) != job.0 {
                continue;
            }
            let result = operation(job.1, job.0, &generation);
            let _gate = gate.lock().unwrap_or_else(|e| e.into_inner());
            if generation.load(Ordering::SeqCst) == job.0 && output.send(result).is_err() {
                break;
            }
        }
    });
    (
        LatestControl {
            sender: Some(sender),
            latest,
            publish,
            handle: Some(handle),
        },
        receiver,
    )
}

/// Derive the second epoch consistently for rendering, export and worker jobs.
pub fn comparison_scene(initial: &Scenario) -> Option<Scenario> {
    initial.compare.then(|| {
        let mut scene = initial.clone();
        scene.date.year = scene.comparison_year;
        scene.compare = false;
        if !valid_date(&scene.date, scene.julian) {
            scene.date.day = 28;
        }
        if scene.scale == TimeScale::UTC {
            scene.scale = TimeScale::UT1;
        }
        scene
    })
}
fn compute_pair(
    engine: &SkyEngine,
    scene: &Scenario,
    generation: u64,
    latest: Option<&AtomicU64>,
) -> Result<SkyPair> {
    let primary = engine.compute(scene, generation, latest, Scope::FullSky, None)?;
    let comparison = comparison_scene(scene)
        .map(|scene| engine.compute(&scene, generation, latest, Scope::FullSky, None))
        .transpose()?;
    Ok(SkyPair {
        primary,
        comparison,
    })
}

pub struct SkyWorker {
    pub receiver: Receiver<Result<SkyPair>>,
    control: LatestControl<Scenario>,
}
impl SkyWorker {
    pub fn new(engine: Arc<SkyEngine>) -> Self {
        let (control, receiver) = latest_worker(move |scene, generation, latest| {
            compute_pair(&engine, &scene, generation, Some(latest))
        });
        Self { receiver, control }
    }
    pub fn request(&self, scene: &Scenario) {
        self.control.request(scene.clone(), &self.receiver);
    }
}

pub struct NavigationWorker {
    pub receiver: Receiver<Result<Option<Scenario>>>,
    control: LatestControl<(Scenario, i32)>,
}
impl NavigationWorker {
    pub fn new(engine: Arc<SkyEngine>) -> Self {
        let (control, receiver) = latest_worker(
            move |(scene, twilight): (Scenario, i32), generation, latest| {
                if twilight == 0 {
                    engine.next_moon_view(&scene, generation, Some(latest))
                } else {
                    engine.twilight_view(
                        &scene,
                        twilight.unsigned_abs() == 1,
                        twilight.signum(),
                        generation,
                        Some(latest),
                    )
                }
            },
        );
        Self { receiver, control }
    }
    pub fn request(&self, scene: &Scenario, twilight: i32) {
        self.control
            .request((scene.clone(), twilight), &self.receiver);
    }
    pub fn cancel(&self) {
        self.control.cancel(&self.receiver);
    }
}

fn prepare_pair(
    pair: &SkyPair,
    scene: &Scenario,
    engine: &SkyEngine,
    width: u32,
    height: u32,
) -> (RenderScene, Option<RenderScene>) {
    let camera = scene.camera(
        if pair.comparison.is_some() {
            (width / 2) as f64
        } else {
            width as f64
        },
        height as f64,
    );
    let primary = render_scene(
        &pair.primary,
        &pair.primary,
        scene,
        &camera,
        Some(&engine.catalog),
    );
    let comparison = pair.comparison.as_ref().map(|sky| {
        render_scene(
            sky,
            sky,
            &sky.scenario,
            &sky.scenario
                .camera((width - width / 2) as f64, height as f64),
            Some(&engine.catalog),
        )
    });
    (primary, comparison)
}
fn is_cancelled(cancel: Option<&AtomicU64>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::SeqCst) != 0)
}
fn check_cancel(recording: &mut Recording, cancel: Option<&AtomicU64>) -> Result<()> {
    if is_cancelled(cancel) {
        recording.cancel()?;
        bail!("Sequence export cancelled; saved PNG frames were preserved");
    }
    Ok(())
}

/// Export every frame from a complete snapshot at its exact requested time.
/// Cancellation keeps completed PNGs and marks the existing manifest cancelled.
pub fn export_sequence(
    engine: &SkyEngine,
    renderer: &mut Renderer,
    initial: &Scenario,
    request: SequenceRequest,
    width: u32,
    height: u32,
    progress: Option<&dyn Fn(i32)>,
    cancel: Option<&AtomicU64>,
) -> Result<()> {
    ensure!(
        width > 0 && height > 0 && (!initial.compare || width >= 2),
        "Invalid sequence output dimensions"
    );
    let mut recording = Recording::new(request, initial.clone())?;
    let mut locked = None;
    while recording.completed < recording.request.frames {
        check_cancel(&mut recording, cancel)?;
        let mut scene = sequence_scene(initial, &recording.request, recording.completed)?;
        if let Some(exposure) = locked {
            scene.auto_exposure = false;
            scene.adaptive_exposure = false;
            scene.exposure = exposure;
        }
        let pair = compute_pair(engine, &scene, 0, cancel)?;
        check_cancel(&mut recording, cancel)?;
        let (primary, comparison) = prepare_pair(&pair, &scene, engine, width, height);
        let mut image = renderer.render(
            &primary,
            comparison.as_ref(),
            width,
            height,
            1.0 / recording.request.fps as f64,
        )?;
        if recording.completed == 0 && recording.request.lock_exposure {
            if scene.adaptive_exposure {
                // Settle the first view without recomputing stellar astrometry.
                for _ in 0..60 {
                    check_cancel(&mut recording, cancel)?;
                    image = renderer.render(&primary, comparison.as_ref(), width, height, 0.1)?;
                }
            }
            locked = Some(renderer.effective_exposure as f64 / 40.0);
        }
        check_cancel(&mut recording, cancel)?;
        let path = recording.frame_path();
        image.save(&path)?;
        save_scenario(
            &scene,
            path.with_extension("json"),
            &pair.primary.data_id,
            Some(&pair.primary.time),
            renderer.effective_exposure as f64,
        )?;
        recording.captured()?;
        if let Some(progress) = progress {
            progress(recording.completed);
        }
    }
    loop {
        check_cancel(&mut recording, cancel)?;
        if recording.poll()? {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    ensure!(
        renderer.gpu.errors() == 0,
        "Vulkan validation errors during sequence export"
    );
    Ok(())
}

#[derive(Debug)]
pub enum SequenceProgress {
    Frame(i32),
    Finished(Result<()>),
}
pub struct SequenceJob {
    pub receiver: Receiver<SequenceProgress>,
    cancelled: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}
impl SequenceJob {
    pub fn new(
        engine: Arc<SkyEngine>,
        data: PathBuf,
        initial: Scenario,
        request: SequenceRequest,
        width: u32,
        height: u32,
        validation: bool,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicU64::new(0));
        let flag = cancelled.clone();
        let handle = thread::spawn(move || {
            let progress = |completed| {
                let _ = sender.send(SequenceProgress::Frame(completed));
            };
            let result = (|| {
                let mut renderer = Renderer::new(&data, validation)?;
                export_sequence(
                    &engine,
                    &mut renderer,
                    &initial,
                    request,
                    width,
                    height,
                    Some(&progress),
                    Some(&flag),
                )
            })();
            let _ = sender.send(SequenceProgress::Finished(result));
        });
        Self {
            receiver,
            cancelled,
            handle: Some(handle),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(1, Ordering::SeqCst);
    }
}
impl Drop for SequenceJob {
    fn drop(&mut self) {
        self.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn superseded_jobs_never_publish_and_queued_results_are_replaced() {
        let (control, receiver) = latest_worker(|number: u64, generation, latest| {
            for _ in 0..10 {
                if latest.load(Ordering::SeqCst) != generation {
                    bail!("Cancelled");
                }
                thread::sleep(Duration::from_millis(1));
            }
            Ok(number)
        });
        control.request(1, &receiver);
        control.request(2, &receiver);
        control.request(3, &receiver);
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap(),
            3
        );
        assert!(receiver.try_recv().is_err());
        control.request(4, &receiver);
        thread::sleep(Duration::from_millis(100));
        control.request(5, &receiver);
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap(),
            5
        );
    }
    #[test]
    fn comparison_changes_epoch_and_uses_ut1_for_utc() {
        let scene = Scenario {
            compare: true,
            comparison_year: -1000,
            scale: TimeScale::UTC,
            ..Scenario::default()
        };
        let comparison = comparison_scene(&scene).unwrap();
        assert_eq!(comparison.date.year, -1000);
        assert_eq!(comparison.scale, TimeScale::UT1);
        assert!(!comparison.compare);
        assert_eq!(comparison.longitude, scene.longitude);
        assert_eq!(comparison.latitude, scene.latitude);
        let mut leap_day = scene;
        leap_day.date.year = 2024;
        leap_day.date.month = 2;
        leap_day.date.day = 29;
        leap_day.comparison_year = 2025;
        let comparison = comparison_scene(&leap_day).unwrap();
        assert_eq!(comparison.date.day, 28);
        assert!(valid_date(&comparison.date, comparison.julian));
    }
    #[test]
    fn cancellation_discards_completed_and_running_navigation_work() {
        let (control, receiver) = latest_worker(|number: u64, generation, latest| {
            for _ in 0..20 {
                if latest.load(Ordering::SeqCst) != generation {
                    bail!("Cancelled");
                }
                thread::sleep(Duration::from_millis(1));
            }
            Ok(number)
        });
        control.request(1, &receiver);
        control.cancel(&receiver);
        thread::sleep(Duration::from_millis(30));
        assert!(receiver.try_recv().is_err());
        control.request(2, &receiver);
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap(),
            2
        );
    }
    #[test]
    #[ignore = "requires a Vulkan device and full runtime assets"]
    fn sequence_export_keeps_exact_times_and_saved_frames_on_cancel() {
        let data = crate::source_data();
        let engine = SkyEngine::new(&data).unwrap();
        let mut renderer = Renderer::new(&data, false).unwrap();
        let base = std::env::temp_dir().join(format!(
            "astra-jobs-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let scene = Scenario {
            magnitude: -2.0,
            atmosphere: false,
            milky_way: false,
            scale: TimeScale::UT1,
            ..Scenario::default()
        };
        let request = SequenceRequest {
            directory: base.join("complete"),
            frames: 2,
            video: false,
            step_seconds: 60.0,
            ..SequenceRequest::default()
        };
        export_sequence(
            &engine,
            &mut renderer,
            &scene,
            request.clone(),
            32,
            16,
            None,
            None,
        )
        .unwrap();
        let expected = sequence_scene(&scene, &request, 1).unwrap();
        let actual =
            crate::scenario::load_scenario(request.directory.join("frame-000001.json"), "")
                .unwrap();
        assert_eq!(actual.date, expected.date);
        let image = image::open(request.directory.join("frame-000001.png")).unwrap();
        assert_eq!((image.width(), image.height()), (32, 16));
        let cancel = AtomicU64::new(0);
        let stop = |completed| {
            if completed == 1 {
                cancel.store(1, Ordering::SeqCst);
            }
        };
        let request = SequenceRequest {
            directory: base.join("cancelled"),
            frames: 3,
            video: false,
            ..SequenceRequest::default()
        };
        assert!(
            export_sequence(
                &engine,
                &mut renderer,
                &scene,
                request.clone(),
                32,
                16,
                Some(&stop),
                Some(&cancel)
            )
            .is_err()
        );
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(request.directory.join("sequence.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["cancelled"], true);
        assert_eq!(manifest["completed_frames"], 1);
        assert!(request.directory.join("frame-000000.png").exists());
        assert!(!request.directory.join("frame-000001.png").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
}
