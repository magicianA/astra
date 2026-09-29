use crate::scenario::Scenario;
use crate::sequence::{
    SequenceRequest, accumulate_trails, save_sequence_manifest, sequence_scene, validate_sequence,
    write_trails,
};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct Recording {
    encoder: Option<Child>,
    trails: Vec<u8>,
    width: u32,
    height: u32,
    done: bool,
    pub request: SequenceRequest,
    pub initial: Scenario,
    pub completed: i32,
}
impl Recording {
    pub fn new(mut request: SequenceRequest, initial: Scenario) -> Result<Self> {
        validate_sequence(&request)?;
        sequence_scene(&initial, &request, request.frames - 1)?;
        if !request.directory.is_absolute() {
            request.directory = std::env::current_dir()?.join(&request.directory);
        }
        if request.directory.exists() && std::fs::read_dir(&request.directory)?.next().is_some() {
            bail!("Export directory must be empty");
        }
        std::fs::create_dir_all(&request.directory)?;
        save_sequence_manifest(&request, &initial, 0, false)?;
        Ok(Self {
            encoder: None,
            trails: Vec::new(),
            width: 0,
            height: 0,
            done: false,
            request,
            initial,
            completed: 0,
        })
    }
    pub fn frame_path(&self) -> PathBuf {
        self.request
            .directory
            .join(format!("frame-{:06}.png", self.completed))
    }
    pub fn encoding(&self) -> bool {
        self.encoder.is_some()
    }
    pub fn captured(&mut self) -> Result<()> {
        if self.done || self.encoder.is_some() || self.completed >= self.request.frames {
            bail!("Recording is not capturing");
        }
        let frame_path = self.frame_path();
        if self.request.trails {
            accumulate_trails(
                &mut self.trails,
                &frame_path,
                &mut self.width,
                &mut self.height,
            )?;
        } else {
            let (width, height) = image::ImageReader::open(self.frame_path())?.into_dimensions()?;
            if self.completed > 0 && (width != self.width || height != self.height) {
                bail!("Window size changed during export");
            }
            self.width = width;
            self.height = height;
        }
        self.completed += 1;
        save_sequence_manifest(&self.request, &self.initial, self.completed, false)?;
        if self.completed < self.request.frames {
            return Ok(());
        }
        if self.request.trails {
            write_trails(
                &self.trails,
                self.width,
                self.height,
                self.request.directory.join("trails.png"),
            )?;
        }
        if !self.request.video {
            self.done = true;
            return Ok(());
        }
        let executable = std::env::var_os("ASTRA_FFMPEG")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    for path in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
                        let path = PathBuf::from(path);
                        if path.exists() {
                            return path;
                        }
                    }
                }
                PathBuf::from("ffmpeg")
            });
        self.encoder = Some(Command::new(executable).args(["-nostdin","-n","-loglevel","error","-framerate"])
            .arg(self.request.fps.to_string()).args(["-start_number","0","-i"]).arg(self.request.directory.join("frame-%06d.png"))
            .arg("-frames:v").arg(self.request.frames.to_string()).args(["-vf","pad=ceil(iw/2)*2:ceil(ih/2)*2","-c:v","libx264","-crf","18","-pix_fmt","yuv420p","-movflags","+faststart"])
            .arg(self.request.directory.join("timelapse.mp4")).stdin(Stdio::null()).spawn()
            .context("Cannot start FFmpeg; PNG frames were saved. Install FFmpeg or set ASTRA_FFMPEG.")?);
        Ok(())
    }
    pub fn poll(&mut self) -> Result<bool> {
        if let Some(child) = self.encoder.as_mut() {
            let Some(status) = child.try_wait()? else {
                return Ok(false);
            };
            self.encoder = None;
            if !status.success() {
                bail!("FFmpeg encoding failed; PNG frames were saved");
            }
            self.done = true;
        }
        Ok(self.done)
    }
    pub fn cancel(&mut self) -> Result<()> {
        if let Some(mut child) = self.encoder.take() {
            // A completed encoder may race with cancel; wait still reaps it.
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        save_sequence_manifest(&self.request, &self.initial, self.completed, true)?;
        self.done = true;
        Ok(())
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "astra-rust-sequence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn frame_manifest_trails_cancel_and_no_overwrite() {
        let directory = directory();
        let initial = Scenario::default();
        let mut request = SequenceRequest {
            directory: directory.join("frames"),
            frames: 2,
            video: false,
            trails: true,
            ..SequenceRequest::default()
        };
        let red = [180, 20, 10, 255, 0, 0, 0, 255];
        let blue = [0, 0, 255, 255, 30, 40, 60, 255];
        let mut recording = Recording::new(request.clone(), initial.clone()).unwrap();
        write_trails(&red, 2, 1, recording.frame_path()).unwrap();
        recording.captured().unwrap();
        write_trails(&blue, 2, 1, recording.frame_path()).unwrap();
        recording.captured().unwrap();
        assert!(recording.poll().unwrap());
        assert_eq!(recording.completed, 2);
        let composite = image::open(request.directory.join("trails.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(composite.as_raw(), &[180, 20, 10, 255, 30, 40, 60, 255]);
        assert!(Recording::new(request.clone(), initial.clone()).is_err());
        request.directory = directory.join("cancelled");
        let mut recording = Recording::new(request.clone(), initial).unwrap();
        write_trails(&red, 2, 1, recording.frame_path()).unwrap();
        recording.captured().unwrap();
        recording.cancel().unwrap();
        let j: serde_json::Value = serde_json::from_slice(
            &std::fs::read(request.directory.join("sequence.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(j["cancelled"], true);
        assert_eq!(j["completed_frames"], 1);
        assert!(request.directory.join("frame-000000.png").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn frame_resize_is_rejected_and_drop_marks_cancelled() {
        let directory = directory();
        let request = SequenceRequest {
            directory: directory.clone(),
            frames: 2,
            video: false,
            ..SequenceRequest::default()
        };
        {
            let mut recording = Recording::new(request, Scenario::default()).unwrap();
            write_trails(&[0, 0, 0, 255], 1, 1, recording.frame_path()).unwrap();
            recording.captured().unwrap();
            write_trails(&[0, 0, 0, 255, 0, 0, 0, 255], 2, 1, recording.frame_path()).unwrap();
            assert!(recording.captured().is_err());
        }
        let j: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("sequence.json")).unwrap())
                .unwrap();
        assert_eq!(j["cancelled"], true);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn ffmpeg_exports_exact_frames_and_pads_odd_dimensions() {
        let executable = std::env::var_os("ASTRA_FFMPEG")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                for path in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
                    if PathBuf::from(path).exists() {
                        return path.into();
                    }
                }
                PathBuf::from("ffmpeg")
            });
        if Command::new(&executable)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let directory = directory();
        let request = SequenceRequest {
            directory: directory.clone(),
            frames: 2,
            video: true,
            ..SequenceRequest::default()
        };
        let mut recording = Recording::new(request, Scenario::default()).unwrap();
        write_trails(
            &[180, 20, 10, 255, 0, 0, 0, 255],
            2,
            1,
            recording.frame_path(),
        )
        .unwrap();
        recording.captured().unwrap();
        write_trails(
            &[0, 0, 255, 255, 30, 40, 60, 255],
            2,
            1,
            recording.frame_path(),
        )
        .unwrap();
        recording.captured().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !recording.poll().unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "FFmpeg export timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            std::fs::metadata(directory.join("timelapse.mp4"))
                .unwrap()
                .len()
                > 0
        );
        assert!(
            directory.join("frame-000000.png").exists()
                && directory.join("frame-000001.png").exists()
        );
        let probe = executable.with_file_name("ffprobe");
        if let Ok(output) = Command::new(probe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name,width,height,nb_frames",
                "-of",
                "json",
            ])
            .arg(directory.join("timelapse.mp4"))
            .output()
        {
            assert!(output.status.success());
            let j: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(j["streams"][0]["codec_name"], "h264");
            assert_eq!(j["streams"][0]["width"], 2);
            assert_eq!(j["streams"][0]["height"], 2);
            assert_eq!(j["streams"][0]["nb_frames"], "2");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
