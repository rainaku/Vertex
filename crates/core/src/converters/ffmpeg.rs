use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter, DetectedInput};
use crate::detect::detect_format;
use crate::error::{Result, VertexError};
use crate::format::{Category, Format};
use crate::options::Options;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

type BinaryCache = Mutex<Option<(Instant, Option<PathBuf>)>>;
static FFMPEG_PATH: OnceLock<BinaryCache> = OnceLock::new();
static FFPROBE_PATH: OnceLock<BinaryCache> = OnceLock::new();

pub fn find_ffmpeg() -> Option<PathBuf> {
    cached_binary(&FFMPEG_PATH, "ffmpeg")
}
pub fn find_ffprobe() -> Option<PathBuf> {
    cached_binary(&FFPROBE_PATH, "ffprobe")
}

fn cached_binary(cache: &OnceLock<BinaryCache>, name: &str) -> Option<PathBuf> {
    let mut cached = cache
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some((checked, path)) = &*cached {
        if path.as_ref().is_some_and(|p| p.is_file())
            || (path.is_none() && checked.elapsed() < Duration::from_secs(2))
        {
            return path.clone();
        }
    }
    let found = find_binary(name);
    *cached = Some((Instant::now(), found.clone()));
    found
}

fn find_binary(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let mut candidates = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&paths)
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.join(&exe)),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local);
        candidates.push(local.join("Microsoft/WinGet/Links").join(&exe));
        candidates.push(local.join("Vertex/bin").join(&exe));
        let packages = local.join("Microsoft/WinGet/Packages");
        if let Ok(entries) = std::fs::read_dir(packages) {
            for entry in entries.flatten().filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains("ffmpeg")
            }) {
                candidates.push(entry.path().join("bin").join(&exe));
                if let Ok(children) = std::fs::read_dir(entry.path()) {
                    candidates.extend(children.flatten().map(|e| e.path().join("bin").join(&exe)));
                }
            }
        }
    }
    for variable in ["SCOOP", "SCOOP_GLOBAL"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.push(
                PathBuf::from(base)
                    .join("apps/ffmpeg/current/bin")
                    .join(&exe),
            );
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        candidates.push(
            PathBuf::from(profile)
                .join("scoop/apps/ffmpeg/current/bin")
                .join(&exe),
        );
    }
    if let Some(base) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(base).join("Vertex/bin").join(&exe));
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(base) = current.parent() {
            candidates.push(base.join("bin").join(&exe));
            candidates.push(base.join(&exe));
        }
    }
    #[cfg(windows)]
    for base in [
        r"C:\ffmpeg\bin",
        r"C:\Program Files\ffmpeg\bin",
        r"C:\ProgramData\chocolatey\bin",
        r"C:\ProgramData\scoop\apps\ffmpeg\current\bin",
    ] {
        candidates.push(PathBuf::from(base).join(&exe));
    }
    candidates.into_iter().find_map(|p| {
        if !p.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if p.metadata().ok()?.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        std::path::absolute(p).ok()
    })
}

/// Reaps the child on all return paths, including panicking progress callbacks.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn get_media_duration(binary: &Path, input: &Path, cancel: &CancelToken) -> Option<f64> {
    use std::io::Seek;
    let mut log = tempfile::tempfile().ok()?;
    let mut cmd = Command::new(binary);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration:stream=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        "-i",
    ])
    .arg(crate::path_utils::external_path(input))
    .stdin(Stdio::null())
    .stdout(log.try_clone().ok()?)
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = ChildGuard(cmd.spawn().ok()?);
    let started = Instant::now();
    let mut poll_ms = 2u64;
    loop {
        if cancel.is_cancelled() || started.elapsed() > Duration::from_secs(15) {
            return None;
        }
        if let Some(status) = child.0.try_wait().ok()? {
            if !status.success() {
                return None;
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
        poll_ms = (poll_ms * 2).min(20);
    }
    log.rewind().ok()?;
    let mut text = String::new();
    log.take(65536).read_to_string(&mut text).ok()?;
    text.lines()
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .reduce(f64::max)
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
                "Audio & Video cần FFmpeg trong PATH hoặc thư mục bin của Vertex. / Audio and video conversion requires FFmpeg.".to_string(),
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
        self.convert_detected(
            DetectedInput {
                path: input,
                format: detect_format(input)?,
            },
            output,
            opts,
            progress,
            cancel,
        )
    }

    fn convert_detected(
        &self,
        detected: DetectedInput<'_>,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        let input = detected.path;
        let from = detected.format;
        cancel.check()?;
        crate::path_utils::ensure_distinct(input, output)?;
        let opts = opts.normalized();
        let binary = find_ffmpeg()
            .ok_or_else(|| VertexError::EngineMissing("FFmpeg binary not found".into()))?;
        let input = std::path::absolute(input).map_err(|source| VertexError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        let target = Format::from_path_extension(output)
            .ok_or_else(|| VertexError::UnrecognizedFormat(output.to_path_buf()))?;
        if !self.targets(from).contains(&target) {
            return Err(VertexError::NoConversionRoute {
                from: from.to_string(),
                to: target.to_string(),
            });
        }
        progress(0.02);
        let duration = find_ffprobe()
            .and_then(|p| get_media_duration(&p, &input, cancel))
            .unwrap_or(0.0);
        cancel.check()?;
        let staged = crate::path_utils::stage_output(output)?;
        let mut cmd = Command::new(binary);
        cmd.args(["-nostdin", "-y", "-hide_banner", "-loglevel", "error", "-i"])
            .arg(crate::path_utils::external_path(&input));
        let mut args = Vec::new();
        if target.category() == Category::Audio {
            args.extend([
                "-map".into(),
                "0:a:0".into(),
                "-vn".into(),
                "-sn".into(),
                "-dn".into(),
            ]);
            append_audio_codec_args(&mut args, target);
        } else if target == Format::Gif {
            let default_cap = if opts.max_width.is_none() && opts.max_height.is_none() {
                Some(480)
            } else {
                None
            };
            let width = opts
                .max_width
                .or(default_cap)
                .map(|w| format!("min(iw,{})", w.max(1)))
                .unwrap_or_else(|| "iw".into());
            let height = opts
                .max_height
                .or(default_cap)
                .map(|h| format!("min(ih,{})", h.max(1)))
                .unwrap_or_else(|| "ih".into());
            args.extend([
                "-an".into(),
                "-vf".into(),
                format!(
                    "fps=15,scale=w='{width}':h='{height}':force_original_aspect_ratio=decrease:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=single[p];[s1][p]paletteuse=new=1:alpha_threshold={}",
                    opts.gif_alpha_threshold
                ),
            ]);
        } else {
            args.extend([
                "-map".into(),
                "0:V:0".into(),
                "-map".into(),
                "0:a:0?".into(),
                "-sn".into(),
                "-dn".into(),
            ]);
            append_video_codec_args(&mut args, target, &opts);
        }
        if opts.strip_metadata {
            args.extend([
                "-map_metadata".into(),
                "-1".into(),
                "-map_chapters".into(),
                "-1".into(),
            ]);
        }
        cmd.args(args)
            .args(["-threads", "0", "-progress", "pipe:1", "-nostats"])
            .arg(crate::path_utils::external_path(
                &std::path::absolute(&staged).map_err(|source| VertexError::Io {
                    path: output.to_path_buf(),
                    source,
                })?,
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let io = |source| VertexError::Io {
            path: output.to_path_buf(),
            source,
        };
        let mut child = ChildGuard(cmd.spawn().map_err(io)?);
        let stderr = child
            .0
            .stderr
            .take()
            .ok_or_else(|| VertexError::Internal("Missing FFmpeg stderr".into()))?;
        let stderr_reader = std::thread::spawn(move || {
            let mut reader = stderr;
            let mut tail = std::collections::VecDeque::with_capacity(16384);
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &buffer[..n];
                        if chunk.len() >= 16384 {
                            tail.clear();
                            tail.extend(&chunk[chunk.len() - 16384..]);
                        } else {
                            let overflow = (tail.len() + chunk.len()).saturating_sub(16384);
                            tail.drain(..overflow);
                            tail.extend(chunk);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            String::from_utf8_lossy(tail.make_contiguous()).into_owned()
        });
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or_else(|| VertexError::Internal("Missing FFmpeg stdout".into()))?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(16);
        let stdout_reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Some(us) = line
                            .trim()
                            .strip_prefix("out_time_us=")
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            let _ = sender.try_send(us);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });
        // Guard is dropped before staging on an unwind, so Windows can remove
        // the temporary file after the child releases its handle.
        let outcome = loop {
            if cancel.is_cancelled() {
                break Err(VertexError::Cancelled);
            }
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(us) => {
                    let seconds = us as f64 / 1_000_000.0;
                    let value = if duration > 0.0 {
                        seconds / duration
                    } else {
                        1.0 - 1.0 / (1.0 + seconds * 0.1)
                    };
                    progress((value as f32).clamp(0.05, 0.98));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    match child.0.try_wait().map_err(io) {
                        Ok(Some(status)) => {
                            break if status.success() {
                                Ok(())
                            } else {
                                Err(VertexError::Encoding("FFmpeg failed".into()))
                            }
                        }
                        Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                        Err(e) => break Err(e),
                    }
                }
            }
        };
        if outcome.is_err() {
            let _ = child.0.kill();
        }
        let _ = child.0.wait();
        let _ = stdout_reader.join();
        let diagnostic = stderr_reader.join().unwrap_or_default();
        if let Err(error) = outcome {
            return Err(if matches!(error, VertexError::Encoding(_)) {
                VertexError::Encoding(format!("FFmpeg: {}", diagnostic.trim()))
            } else {
                error
            });
        }
        cancel.check()?;
        if staged.metadata().map_err(io)?.len() == 0 {
            return Err(VertexError::Encoding(
                "FFmpeg produced an empty output".into(),
            ));
        }
        crate::path_utils::ensure_distinct(&input, output)?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}

fn append_audio_codec_args(args: &mut Vec<String>, target: Format) {
    if matches!(target, Format::Mp3 | Format::Wma) {
        args.extend(["-ac".into(), "2".into(), "-ar".into(), "48000".into()]);
    }
    if target == Format::Opus {
        args.extend(["-ar".into(), "48000".into()]);
    }
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
    use crate::options::{VideoCodec, VideoPreset};
    let width = opts
        .max_width
        .map(|w| format!("min(iw,{})", w.max(2)))
        .unwrap_or("iw".into());
    let height = opts
        .max_height
        .map(|h| format!("min(ih,{})", h.max(2)))
        .unwrap_or("ih".into());
    let scale = if target == Format::ThreeGp {
        // H.263 supports a fixed set of frame sizes. Fit + pad preserves aspect
        // ratio rather than asking the encoder to accept an arbitrary frame.
        "scale=352:288:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=352:288:(ow-iw)/2:(oh-ih)/2,setsar=1".into()
    } else {
        format!("scale=w='{width}':h='{height}':force_original_aspect_ratio=decrease:force_divisible_by=2")
    };
    args.extend(["-vf".into(), scale, "-pix_fmt".into(), "yuv420p".into()]);
    let audio_rate = format!("{}k", opts.video_audio_kbps);
    match target {
        Format::Webm => args.extend([
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-crf".into(),
            opts.video_crf.to_string(),
            "-b:v".into(),
            "0".into(),
            "-deadline".into(),
            "good".into(),
            "-cpu-used".into(),
            "4".into(),
            "-row-mt".into(),
            "1".into(),
            "-c:a".into(),
            "libopus".into(),
            "-ar".into(),
            "48000".into(),
            "-b:a".into(),
            audio_rate,
        ]),
        Format::Avi => args.extend([
            "-c:v".into(),
            "mpeg4".into(),
            "-q:v".into(),
            "3".into(),
            "-c:a".into(),
            "libmp3lame".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "48000".into(),
            "-q:a".into(),
            "2".into(),
        ]),
        Format::ThreeGp => args.extend([
            "-c:v".into(),
            "h263".into(),
            "-b:v".into(),
            "384k".into(),
            "-c:a".into(),
            "aac".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "44100".into(),
            "-b:a".into(),
            "64k".into(),
        ]),
        _ => {
            let codec = match opts.video_codec {
                VideoCodec::H264 => "libx264",
                VideoCodec::H265 => "libx265",
            };
            let preset = match opts.video_preset {
                VideoPreset::Fast => "fast",
                VideoPreset::Medium => "medium",
                VideoPreset::Slow => "slow",
            };
            args.extend([
                "-c:v".into(),
                codec.into(),
                "-preset".into(),
                preset.into(),
                "-crf".into(),
                opts.video_crf.to_string(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                audio_rate,
            ]);
            if matches!(target, Format::Mp4 | Format::Mov) {
                args.extend(["-movflags".into(), "+faststart".into()]);
                if matches!(opts.video_codec, VideoCodec::H265) {
                    args.extend(["-tag:v".into(), "hvc1".into()]);
                }
            }
        }
    }
}
