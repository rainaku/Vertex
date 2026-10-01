use std::fs::File;
use std::io::BufWriter;
use tempfile::tempdir;

use image::{ImageBuffer, Rgba};
use vertex_core::{
    detect_format, resolve_target_path, CancelToken, CollisionPolicy, Format, Options, Registry,
};

#[test]
fn test_image_roundtrip_conversion() {
    let dir = tempdir().expect("create temp dir");
    let input_path = dir.path().join("sample.png");

    // Create a 64x64 test image with pure red and green pixels
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(64, 64, |x, y| {
        if (x + y) % 2 == 0 {
            Rgba([255, 60, 20, 255])
        } else {
            Rgba([20, 200, 80, 255])
        }
    });

    let f = File::create(&input_path).unwrap();
    img.write_to(&mut BufWriter::new(f), image::ImageFormat::Png)
        .unwrap();

    // 1. Detect format
    let detected = detect_format(&input_path).expect("detect format");
    assert_eq!(detected, Format::Png);

    let registry = Registry::default();
    let cancel = CancelToken::new();
    let opts = Options::default();

    // 2. Convert to WebP
    let out_webp = registry
        .convert(&input_path, Format::Webp, &opts, &|_| {}, &cancel)
        .expect("convert to webp");
    assert!(out_webp.exists());
    assert_eq!(detect_format(&out_webp).unwrap(), Format::Webp);

    // 3. Convert to JPG
    let out_jpg = registry
        .convert(&input_path, Format::Jpg, &opts, &|_| {}, &cancel)
        .expect("convert to jpg");
    assert!(out_jpg.exists());
    assert_eq!(detect_format(&out_jpg).unwrap(), Format::Jpg);

    // 4. Convert to BMP
    let out_bmp = registry
        .convert(&input_path, Format::Bmp, &opts, &|_| {}, &cancel)
        .expect("convert to bmp");
    assert!(out_bmp.exists());
    assert_eq!(detect_format(&out_bmp).unwrap(), Format::Bmp);

    // 5. Convert to AVIF
    let out_avif = registry
        .convert(&input_path, Format::Avif, &opts, &|_| {}, &cancel)
        .expect("convert to avif");
    assert!(out_avif.exists());
    assert_eq!(detect_format(&out_avif).unwrap(), Format::Avif);
}

#[test]
fn test_collision_resolution() {
    let dir = tempdir().expect("create temp dir");
    let input = dir.path().join("photo.png");
    std::fs::write(&input, b"fake png").unwrap();

    let opts = Options {
        output_dir: Some(dir.path().to_path_buf()),
        collision_policy: CollisionPolicy::RenameWithSuffix,
        ..Default::default()
    };

    // First resolution
    let target1 = resolve_target_path(&input, Format::Jpg, &opts).unwrap();
    assert_eq!(target1.file_name().unwrap(), "photo.jpg");
    std::fs::write(&target1, b"first").unwrap();

    // Second resolution should append (1)
    let target2 = resolve_target_path(&input, Format::Jpg, &opts).unwrap();
    assert_eq!(target2.file_name().unwrap(), "photo (1).jpg");
    std::fs::write(&target2, b"second").unwrap();

    // Third resolution should append (2)
    let target3 = resolve_target_path(&input, Format::Jpg, &opts).unwrap();
    assert_eq!(target3.file_name().unwrap(), "photo (2).jpg");
}

#[test]
fn test_audio_and_video_targets() {
    let registry = Registry::default();

    // Check Audio available targets
    let mp3_targets = registry.available_targets(Format::Mp3);
    assert!(!mp3_targets.is_empty(), "MP3 should have available targets");
    let target_fmts: Vec<Format> = mp3_targets.into_iter().map(|(f, _)| f).collect();
    assert!(target_fmts.contains(&Format::Wav));
    assert!(target_fmts.contains(&Format::Flac));
    assert!(target_fmts.contains(&Format::Aac));
    assert!(target_fmts.contains(&Format::Ogg));

    // Check Video available targets (should include both videos & audio extraction)
    let mp4_targets = registry.available_targets(Format::Mp4);
    assert!(!mp4_targets.is_empty(), "MP4 should have available targets");
    let video_target_fmts: Vec<Format> = mp4_targets.into_iter().map(|(f, _)| f).collect();
    // Video targets
    assert!(video_target_fmts.contains(&Format::Webm));
    assert!(video_target_fmts.contains(&Format::Mov));
    assert!(video_target_fmts.contains(&Format::Mkv));
    assert!(video_target_fmts.contains(&Format::Gif));
    // Audio extraction targets
    assert!(video_target_fmts.contains(&Format::Mp3));
    assert!(video_target_fmts.contains(&Format::Wav));

    // Verify video formats come FIRST before audio extraction targets
    assert_eq!(
        video_target_fmts[0].category(),
        vertex_core::Category::Video
    );
    assert_eq!(
        video_target_fmts[1].category(),
        vertex_core::Category::Video
    );
    assert_eq!(
        video_target_fmts[2].category(),
        vertex_core::Category::Video
    );
}

#[test]
fn test_ffmpeg_audio_conversion() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = tempdir().expect("create temp dir");
    let input_wav = dir.path().join("tone.wav");

    // Generate a simple synthetic 1-second 44.1kHz mono WAV file
    let sample_rate = 44100u32;
    let num_samples = 44100u32;
    let mut wav_bytes = Vec::new();
    // RIFF header
    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVE");
    // fmt chunk
    wav_bytes.extend_from_slice(b"fmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes()); // subchunk size
    wav_bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav_bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    wav_bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav_bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
                                                       // data chunk
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());
    for i in 0..num_samples {
        let sample = (f32::sin(i as f32 * 440.0 * 2.0 * std::f32::consts::PI / sample_rate as f32)
            * 16000.0) as i16;
        wav_bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(&input_wav, &wav_bytes).expect("write wav");

    let detected = detect_format(&input_wav).expect("detect wav");
    assert_eq!(detected, Format::Wav);

    let registry = Registry::default();
    let cancel = CancelToken::new();
    let opts = Options::default();

    let progress_called = Arc::new(AtomicBool::new(false));
    let progress_clone = progress_called.clone();

    // Convert WAV -> MP3
    let out_mp3 = registry
        .convert(
            &input_wav,
            Format::Mp3,
            &opts,
            &move |p| {
                if p > 0.0 {
                    progress_clone.store(true, Ordering::SeqCst);
                }
            },
            &cancel,
        )
        .expect("convert wav to mp3");

    assert!(out_mp3.exists(), "MP3 output must exist");
    assert!(
        out_mp3.metadata().unwrap().len() > 100,
        "MP3 output must not be empty"
    );
    assert!(
        progress_called.load(Ordering::SeqCst),
        "Progress callback must be invoked"
    );
}

#[test]
fn test_ffmpeg_video_conversion_and_audio_extraction() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use vertex_core::converters::ffmpeg::find_ffmpeg;

    let ffmpeg_bin = match find_ffmpeg() {
        Some(b) => b,
        None => return, // skip if ffmpeg is not available on test runner
    };

    let dir = tempdir().expect("create temp dir");
    let input_mp4 = dir.path().join("clip.mp4");

    // Generate a minimal 1-second synthetic MP4 clip with video test pattern and audio sine wave
    let mut make_cmd = std::process::Command::new(&ffmpeg_bin);
    make_cmd.args([
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=1:size=160x120:rate=15",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=1",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-c:a",
        "aac",
        "-y",
    ]);
    make_cmd.arg(&input_mp4);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        make_cmd.creation_flags(0x08000000);
    }
    let res = make_cmd.output().expect("generate synthetic mp4");
    assert!(
        res.status.success(),
        "Failed to generate test mp4: {}",
        String::from_utf8_lossy(&res.stderr)
    );

    let detected = detect_format(&input_mp4).expect("detect mp4");
    assert_eq!(detected, Format::Mp4);

    let registry = Registry::default();
    let cancel = CancelToken::new();
    let opts = Options::default();

    // 1. Audio Extraction: MP4 -> MP3
    let progress_mp3 = Arc::new(AtomicBool::new(false));
    let progress_mp3_clone = progress_mp3.clone();
    let out_mp3 = registry
        .convert(
            &input_mp4,
            Format::Mp3,
            &opts,
            &move |p| {
                if p > 0.0 {
                    progress_mp3_clone.store(true, Ordering::SeqCst);
                }
            },
            &cancel,
        )
        .expect("extract audio from mp4 to mp3");

    assert!(out_mp3.exists(), "Extracted MP3 must exist");
    assert!(
        out_mp3.metadata().unwrap().len() > 100,
        "Extracted MP3 must not be empty"
    );
    assert!(
        progress_mp3.load(Ordering::SeqCst),
        "Progress callback must be invoked for extraction"
    );

    // 2. Video Conversion: MP4 -> WEBM
    let progress_webm = Arc::new(AtomicBool::new(false));
    let progress_webm_clone = progress_webm.clone();
    let out_webm = registry
        .convert(
            &input_mp4,
            Format::Webm,
            &opts,
            &move |p| {
                if p > 0.0 {
                    progress_webm_clone.store(true, Ordering::SeqCst);
                }
            },
            &cancel,
        )
        .expect("convert mp4 to webm");

    assert!(out_webm.exists(), "Converted WEBM must exist");
    assert!(
        out_webm.metadata().unwrap().len() > 100,
        "Converted WEBM must not be empty"
    );
    assert!(
        progress_webm.load(Ordering::SeqCst),
        "Progress callback must be invoked for video transcode"
    );

    // 3. MP4 -> MP4 compression / re-encoding
    let out_mp4_compressed = registry
        .convert(&input_mp4, Format::Mp4, &opts, &|_| {}, &cancel)
        .expect("compress mp4 to mp4");
    assert!(out_mp4_compressed.exists(), "Compressed MP4 must exist");
    assert_ne!(
        out_mp4_compressed, input_mp4,
        "Compressed output must not overwrite input path"
    );
    assert!(out_mp4_compressed.metadata().unwrap().len() > 100);
}
