use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter};
use crate::detect::detect_format;
use crate::error::{Result, VertexError};
use crate::format::{Category, Format};
use crate::options::Options;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static FFMPEG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static FFPROBE_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn find_ffmpeg() -> Option<PathBuf> {
    FFMPEG_PATH.get_or_init(|| find_binary("ffmpeg")).clone()
}

pub fn find_ffprobe() -> Option<PathBuf> {
    FFPROBE_PATH.get_or_init(|| find_binary("ffprobe")).clone()
}

fn find_binary(name: &str) -> Option<PathBuf> {
    let exe_name = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    // 1. Check if the binary is runnable directly from PATH
    let mut check_cmd = Command::new(&exe_name);
    check_cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        check_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if let Ok(out) = check_cmd.output() {
        if out.status.success() {
            return Some(PathBuf::from(exe_name));
        }
    }

    // 2. WinGet standard directory paths
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let winget_link = PathBuf::from(&local_app_data)
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join(&exe_name);
        if winget_link.exists() {
            return Some(winget_link);
        }

        // Search inside WinGet Packages
        let packages_dir = PathBuf::from(&local_app_data)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages");
        if packages_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&packages_dir) {
                for entry in entries.flatten() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    if folder_name.contains("FFmpeg") {
                        if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                            for sub in sub_entries.flatten() {
                                let candidate = sub.path().join("bin").join(&exe_name);
                                if candidate.exists() {
                                    return Some(candidate);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Local Vertex bin
        let vertex_local = PathBuf::from(&local_app_data)
            .join("Vertex")
            .join("bin")
            .join(&exe_name);
        if vertex_local.exists() {
            return Some(vertex_local);
        }
    }

    // 3. APPDATA roaming
    if let Ok(app_data) = std::env::var("APPDATA") {
        let vertex_roaming = PathBuf::from(&app_data)
            .join("Vertex")
            .join("bin")
            .join(&exe_name);
        if vertex_roaming.exists() {
            return Some(vertex_roaming);
        }
    }

    // 4. Current application executable folder and its bin/
    if let Ok(curr_exe) = std::env::current_exe() {
        if let Some(parent) = curr_exe.parent() {
            let candidate = parent.join("bin").join(&exe_name);
            if candidate.exists() {
                return Some(candidate);
            }
            let candidate_root = parent.join(&exe_name);
            if candidate_root.exists() {
                return Some(candidate_root);
            }
        }
    }

    // 5. Working directory ./bin/
    let local_bin = PathBuf::from("bin").join(&exe_name);
    if local_bin.exists() {
        return Some(local_bin);
    }

    // 6. Common Windows paths
    for base in &[
        r"C:\ffmpeg\bin",
        r"C:\Program Files\ffmpeg\bin",
        r"C:\ProgramData\chocolatey\bin",
    ] {
        let candidate = PathBuf::from(base).join(&exe_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Probes the duration (in seconds) of a multimedia file using ffprobe.
fn get_media_duration(ffprobe_bin: &Path, input: &Path) -> Option<f64> {
    let mut cmd = Command::new(ffprobe_bin);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
    ]);
    cmd.arg(input);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = std::str::from_utf8(&output.stdout).ok()?;
    s.trim().parse::<f64>().ok()
}

pub struct FfmpegConverter {
    supported_sources: Vec<Format>,
}

impl Default for FfmpegConverter {
    fn default() -> Self {
        Self {
            supported_sources: vec![
                // Video formats
                Format::Mp4,
                Format::Mov,
                Format::Mkv,
                Format::Avi,
                Format::Webm,
                Format::Wmv,
                Format::Flv,
                Format::ThreeGp,
                Format::Ts,
                // Audio formats
                Format::Mp3,
                Format::Wav,
                Format::Flac,
                Format::Aac,
                Format::Ogg,
                Format::M4a,
                Format::Opus,
                Format::Aiff,
                Format::Wma,
                // Animated image
                Format::Gif,
            ],
        }
    }
}

impl Converter for FfmpegConverter {
    fn name(&self) -> &'static str {
        "FfmpegConverter"
    }

    fn sources(&self) -> &[Format] {
        &self.supported_sources
    }

    fn targets(&self, from: Format) -> Vec<Format> {
        match from {
            // Video sources support video conversion and audio extraction.
            Format::Mp4
            | Format::Mov
            | Format::Mkv
            | Format::Avi
            | Format::Webm
            | Format::Wmv
            | Format::Flv
            | Format::ThreeGp
            | Format::Ts => {
                let targets = vec![
                    // Video outputs
                    Format::Mp4,
                    Format::Mov,
                    Format::Mkv,
                    Format::Avi,
                    Format::Webm,
                    Format::ThreeGp,
                    Format::Ts,
                    Format::Gif,
                    Format::Mp3,
                    Format::Wav,
                    Format::Flac,
                    Format::Aac,
                    Format::M4a,
                    Format::Ogg,
                    Format::Opus,
                    Format::Aiff,
                    Format::Wma,
                ];
                targets
            }

            // Audio sources convert to other audio formats
            Format::Mp3
            | Format::Wav
            | Format::Flac
            | Format::Aac
            | Format::Ogg
            | Format::M4a
            | Format::Opus
            | Format::Aiff
            | Format::Wma => {
                let targets = vec![
                    Format::Mp3,
                    Format::Wav,
                    Format::Flac,
                    Format::Aac,
                    Format::Ogg,
                    Format::M4a,
                    Format::Opus,
                    Format::Aiff,
                    Format::Wma,
                ];
                targets
            }

            // GIF can convert to modern video
            Format::Gif => vec![Format::Mp4, Format::Webm],

            _ => Vec::new(),
        }
    }

    fn available(&self) -> Availability {
        if find_ffmpeg().is_some() {
            Availability::Ok
        } else {
            Availability::Missing(
                "Cần FFmpeg để xử lý Audio & Video. Cài đặt nhanh qua winget: winget install Gyan.FFmpeg.Essentials".to_string(),
            )
        }
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        progress(0.02);

        let ffmpeg_bin = find_ffmpeg().ok_or_else(|| {
            VertexError::EngineMissing("FFmpeg binary not found on the system".to_string())
        })?;

        let duration_secs = find_ffprobe()
            .and_then(|probe| get_media_duration(&probe, input))
            .unwrap_or(0.0);

        let from_format = detect_format(input)?;
        let to_format = Format::from_extension(
            output
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default(),
        )
        .ok_or_else(|| VertexError::UnrecognizedFormat(output.to_path_buf()))?;

        if !self.targets(from_format).contains(&to_format) {
            return Err(VertexError::NoConversionRoute {
                from: from_format.to_string(),
                to: to_format.to_string(),
            });
        }

        // Build command args for MAXIMUM PERFORMANCE
        let mut args: Vec<String> = vec!["-nostdin".into(), "-y".into(), "-hide_banner".into()];

        // Hardware acceleration for video inputs (auto-negotiates NVDEC/D3D11VA/DXVA2/QSV)
        if to_format.category() != Category::Audio
            && (matches!(from_format.category(), Category::Video) || from_format == Format::Gif)
        {
            args.push("-hwaccel".into());
            args.push("auto".into());
        }

        // Input file
        args.push("-i".into());
        args.push(input.to_string_lossy().to_string());

        // Stream and codec mapping
        if from_format.category() == Category::Video && to_format.category() == Category::Audio {
            args.extend(["-map".into(), "0:a:0".into(), "-vn".into()]);
            append_audio_codec_args(&mut args, to_format);
        } else if matches!(from_format.category(), Category::Audio)
            && matches!(to_format.category(), Category::Audio)
        {
            // Audio to audio transcoding
            append_audio_codec_args(&mut args, to_format);
        } else if from_format == Format::Gif && matches!(to_format.category(), Category::Video) {
            append_video_codec_args(&mut args, to_format, opts);
        } else if matches!(from_format.category(), Category::Video) {
            // Video to video or Video to GIF
            if to_format == Format::Gif {
                // High-performance single-pass palette generation for GIF
                args.extend([
                    "-vf".into(),
                    "fps=15,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".into(),
                ]);
            } else {
                append_video_codec_args(&mut args, to_format, opts);
            }
        }

        // Multithreading
        args.push("-threads".into());
        args.push("0".into());

        // Progress reporting protocol
        args.push("-progress".into());
        args.push("pipe:1".into());
        args.push("-nostats".into());

        // Output destination
        args.push(output.to_string_lossy().to_string());

        // Spawn child process
        let mut cmd = Command::new(&ffmpeg_bin);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| VertexError::Io {
            path: output.to_path_buf(),
            source: e,
        })?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| VertexError::Internal("Missing FFmpeg stderr".into()))?;
        let stderr_reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = stderr;
            let mut tail = Vec::new();
            let mut buffer = [0u8; 4096];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                tail.extend_from_slice(&buffer[..count]);
                if tail.len() > 16384 {
                    tail.drain(..tail.len() - 16384);
                }
            }
            String::from_utf8_lossy(&tail).into_owned()
        });

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| VertexError::Internal("Failed to capture FFmpeg stdout".to_string()))?;

        let (sender, receiver) = std::sync::mpsc::sync_channel(64);
        let stdout_reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });

        // Monitor progress and cancellation in real time
        loop {
            if cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                drop(receiver);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                let _ = std::fs::remove_file(output);
                return Err(VertexError::Cancelled);
            }

            match receiver.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if let Some((k, v)) = trimmed.split_once('=') {
                        match k {
                            "out_time_us" => {
                                if let Ok(us) = v.parse::<u64>() {
                                    let current_secs = us as f64 / 1_000_000.0;
                                    if duration_secs > 0.0 {
                                        let p = ((current_secs / duration_secs) as f32)
                                            .clamp(0.05, 0.98);
                                        progress(p);
                                    } else {
                                        // Incremental estimation if duration unknown
                                        let est = (1.0 - (1.0 / (1.0 + current_secs * 0.1))) as f32;
                                        progress(est.clamp(0.05, 0.95));
                                    }
                                }
                            }
                            "progress" if v == "end" => {
                                progress(0.99);
                            }
                            _ => {}
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = stdout_reader.join();
        while child
            .try_wait()
            .map_err(|source| VertexError::Io {
                path: output.to_path_buf(),
                source,
            })?
            .is_none()
        {
            if cancel.is_cancelled() {
                let _ = child.kill();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let status = child.wait().map_err(|e| VertexError::Io {
            path: output.to_path_buf(),
            source: e,
        })?;

        let err_msg = stderr_reader.join().unwrap_or_default();
        cancel.check()?;
        if !status.success() {
            let _ = std::fs::remove_file(output);
            return Err(VertexError::Encoding(format!(
                "FFmpeg conversion failed: {}",
                err_msg.lines().last().unwrap_or("unknown error")
            )));
        }

        // Verify output file was produced
        if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
            return Err(VertexError::Encoding(
                "Output file was not created or is empty".to_string(),
            ));
        }

        progress(1.0);
        Ok(())
    }
}

fn append_audio_codec_args(args: &mut Vec<String>, target: Format) {
    match target {
        Format::Mp3 => {
            args.extend([
                "-c:a".into(),
                "libmp3lame".into(),
                "-q:a".into(),
                "2".into(),
            ]);
        }
        Format::Wav => {
            args.extend(["-c:a".into(), "pcm_s16le".into()]);
        }
        Format::Flac => {
            args.extend([
                "-c:a".into(),
                "flac".into(),
                "-compression_level".into(),
                "5".into(),
            ]);
        }
        Format::Aac | Format::M4a => {
            args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "256k".into()]);
        }
        Format::Ogg => {
            args.extend(["-c:a".into(), "libvorbis".into(), "-q:a".into(), "6".into()]);
        }
        Format::Opus => {
            args.extend([
                "-c:a".into(),
                "libopus".into(),
                "-b:a".into(),
                "128k".into(),
            ]);
        }
        Format::Aiff => {
            args.extend(["-c:a".into(), "pcm_s16be".into()]);
        }
        Format::Wma => {
            args.extend(["-c:a".into(), "wmav2".into(), "-b:a".into(), "192k".into()]);
        }
        _ => {
            args.extend([
                "-c:a".into(),
                "libmp3lame".into(),
                "-q:a".into(),
                "2".into(),
            ]);
        }
    }
}

fn append_video_codec_args(args: &mut Vec<String>, target: Format, opts: &Options) {
    if matches!(target, Format::Mp4 | Format::Mov | Format::Mkv | Format::Ts) {
        use crate::options::{VideoCodec, VideoPreset};
        let codec = match opts.video_codec {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
        };
        let preset = match opts.video_preset {
            VideoPreset::Fast => "fast",
            VideoPreset::Medium => "medium",
            VideoPreset::Slow => "slow",
        };
        let width = opts
            .max_width
            .map(|w| format!("min(iw,{})", w.max(2)))
            .unwrap_or("iw".into());
        let height = opts
            .max_height
            .map(|h| format!("min(ih,{})", h.max(2)))
            .unwrap_or("ih".into());
        // Keep aspect ratio, avoid upscaling, and satisfy 4:2:0 even dimensions.
        args.extend([
            "-vf".into(), format!("scale=w='{width}':h='{height}':force_original_aspect_ratio=decrease:force_divisible_by=2"),
            "-c:v".into(), codec.into(), "-preset".into(), preset.into(),
            "-crf".into(), opts.video_crf.clamp(18, 35).to_string(),
            "-c:a".into(), "aac".into(), "-b:a".into(), format!("{}k", opts.video_audio_kbps.clamp(64, 320)),
        ]);
        if matches!(target, Format::Mp4 | Format::Mov) {
            args.extend(["-movflags".into(), "+faststart".into()]);
            if matches!(opts.video_codec, VideoCodec::H265) {
                args.extend(["-tag:v".into(), "hvc1".into()]);
            }
        }
        return;
    }
    // Resize video if options specify max dimensions
    if let (Some(w), Some(h)) = (opts.max_width, opts.max_height) {
        args.extend([
            "-vf".into(),
            format!("scale=w={}:h={}:force_original_aspect_ratio=decrease", w, h),
        ]);
    } else if let Some(w) = opts.max_width {
        args.extend(["-vf".into(), format!("scale='min({},iw)':-2", w)]);
    } else if let Some(h) = opts.max_height {
        args.extend(["-vf".into(), format!("scale=-2:'min({},ih)'", h)]);
    }

    match target {
        Format::Webm => {
            args.extend([
                "-c:v".into(),
                "libvpx-vp9".into(),
                "-crf".into(),
                "30".into(),
                "-b:v".into(),
                "0".into(),
                "-deadline".into(),
                "realtime".into(),
                "-cpu-used".into(),
                "4".into(),
                "-row-mt".into(),
                "1".into(),
                "-c:a".into(),
                "libopus".into(),
                "-b:a".into(),
                "128k".into(),
            ]);
        }
        Format::Avi => {
            args.extend([
                "-c:v".into(),
                "mpeg4".into(),
                "-q:v".into(),
                "3".into(),
                "-c:a".into(),
                "libmp3lame".into(),
                "-q:a".into(),
                "2".into(),
            ]);
        }
        Format::ThreeGp => {
            args.extend([
                "-c:v".into(),
                "h263".into(),
                "-b:v".into(),
                "384k".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "64k".into(),
            ]);
        }
        _ => {
            // Default to fast MP4
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                "22".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
        }
    }
}
