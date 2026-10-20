use std::path::Path;
use std::process::Command;
use vertex_core::converters::ffmpeg::{find_ffmpeg, find_ffprobe, FfmpegConverter};
use vertex_core::{CancelToken, Converter, Format, Options, Registry};

fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

fn make(binary: &Path, args: &[&str], output: &Path) {
    let result = command(binary)
        .args(["-nostdin", "-y", "-hide_banner", "-loglevel", "error"])
        .args(args)
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "fixture: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn video_gif_conversion_applies_dimensions_and_the_alpha_threshold() {
    let Some(binary) = find_ffmpeg() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let frame = dir.path().join("alpha.png");
    image::RgbaImage::from_pixel(40, 20, image::Rgba([255, 0, 0, 128]))
        .save(&frame)
        .unwrap();
    let input = dir.path().join("alpha.mov");
    make(
        &binary,
        &[
            "-loop",
            "1",
            "-i",
            frame.to_str().unwrap(),
            "-t",
            "0.2",
            "-c:v",
            "png",
            "-pix_fmt",
            "rgba",
        ],
        &input,
    );
    for (threshold, expected_alpha) in [(1, 255), (200, 0)] {
        let options = Options {
            max_width: Some(20),
            max_height: Some(20),
            gif_alpha_threshold: threshold,
            ..Default::default()
        };
        let output = Registry::default()
            .convert(&input, Format::Gif, &options, &|_| {}, &CancelToken::new())
            .unwrap();
        let image = image::open(output).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (20, 10));
        assert_eq!(image.get_pixel(10, 5)[3], expected_alpha);
    }
}

#[test]
fn audio_cover_art_and_surround_high_sample_rates_convert() {
    let Some(binary) = find_ffmpeg() else {
        eprintln!("FFmpeg absent: skipping media integration test");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let cover = dir.path().join("cover.png");
    image::RgbImage::from_pixel(32, 32, image::Rgb([255, 0, 0]))
        .save(&cover)
        .unwrap();
    let cover_text = cover.to_str().unwrap();
    let input = dir.path().join("surround with cover.flac");
    make(
        &binary,
        &[
            "-f",
            "lavfi",
            "-i",
            "anullsrc=channel_layout=5.1:sample_rate=96000",
            "-i",
            cover_text,
            "-t",
            "0.2",
            "-map",
            "0:a",
            "-map",
            "1:v",
            "-c:a",
            "flac",
            "-c:v",
            "png",
            "-disposition:v",
            "attached_pic",
        ],
        &input,
    );
    let registry = Registry::default();
    for target in [
        Format::Mp3,
        Format::Wav,
        Format::Aac,
        Format::Opus,
        Format::Aiff,
        Format::Wma,
    ] {
        let output = registry
            .convert(
                &input,
                target,
                &Options::default(),
                &|_| {},
                &CancelToken::new(),
            )
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        assert!(output.metadata().unwrap().len() > 0);
        if let Some(probe) = find_ffprobe() {
            let result = command(&probe)
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "v",
                    "-show_entries",
                    "stream=index",
                    "-of",
                    "csv=p=0",
                    "-i",
                ])
                .arg(output)
                .output()
                .unwrap();
            assert!(
                result.stdout.is_empty(),
                "Audio output must not include cover video streams"
            );
        }
    }
}

#[test]
fn odd_video_frames_and_h263_output_use_compatible_geometry() {
    let Some(binary) = find_ffmpeg() else {
        eprintln!("FFmpeg absent: skipping media integration test");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("odd 161x121.mkv");
    make(
        &binary,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.2:size=161x121:rate=10",
            "-c:v",
            "ffv1",
        ],
        &input,
    );
    let registry = Registry::default();
    for target in [Format::Mp4, Format::Webm, Format::Avi, Format::ThreeGp] {
        let opts = Options {
            max_width: Some(151),
            max_height: Some(111),
            ..Options::default()
        };
        let output = registry
            .convert(&input, target, &opts, &|_| {}, &CancelToken::new())
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        if let Some(probe) = find_ffprobe() {
            let result = command(&probe)
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=width,height,pix_fmt",
                    "-of",
                    "json",
                    "-i",
                ])
                .arg(output)
                .output()
                .unwrap();
            let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            let stream = &json["streams"][0];
            assert_eq!(stream["pix_fmt"], "yuv420p");
            let (w, h) = (
                stream["width"].as_u64().unwrap(),
                stream["height"].as_u64().unwrap(),
            );
            assert_eq!((w % 2, h % 2), (0, 0));
            if target == Format::ThreeGp {
                assert_eq!((w, h), (352, 288));
            } else {
                assert!(w <= 151 && h <= 111);
            }
        }
    }
}

#[test]
fn direct_ffmpeg_conversion_preserves_source_and_cleans_failures_and_cancellation() {
    let Some(binary) = find_ffmpeg() else {
        eprintln!("FFmpeg absent: skipping media integration test");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("-âm thanh.wav");
    make(
        &binary,
        &["-f", "lavfi", "-i", "sine=frequency=440:duration=2"],
        &input,
    );
    let original = std::fs::read(&input).unwrap();
    let converter = FfmpegConverter::default();
    assert!(converter
        .convert(
            &input,
            &input,
            &Options::default(),
            &|_| {},
            &CancelToken::new()
        )
        .is_err());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let output = dir.path().join("target.mp3");
    std::fs::write(&output, b"existing output").unwrap();
    let cancel = CancelToken::new();
    let result = converter.convert(
        &input,
        &output,
        &Options::default(),
        &|p| {
            if p >= 0.05 {
                cancel.cancel();
            }
        },
        &cancel,
    );
    assert!(matches!(result, Err(vertex_core::VertexError::Cancelled)));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing output");
    let broken = dir.path().join("broken.wav");
    std::fs::write(&broken, b"broken wav contents").unwrap();
    let result = converter.convert(
        &broken,
        &output,
        &Options::default(),
        &|_| {},
        &CancelToken::new(),
    );
    let message = result.unwrap_err().to_string();
    assert!(
        message.contains("Invalid data") || message.contains("Error"),
        "{message}"
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"existing output");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
}
