use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use vertex_core::{detect_format, CancelToken, Format, Options, Registry};
use vertex_core::{Availability, CollisionPolicy, Converter, SafeLauncher, SensitiveDataScrubber};

#[test]
fn text_and_container_detection() {
    let dir = tempdir().unwrap();
    let cases: &[(&str, &[u8], Format)] = &[
        ("drawing.txt", b"\xef\xbb\xbf<?xml version=\"1.0\"?><!-- exported --><svg xmlns=\"http://www.w3.org/2000/svg\"/>", Format::Svg),
        ("page.txt", b"<!doctype html><html></html>", Format::Html),
        ("archive.TAR.GZ", b"\x1f\x8b\x08", Format::TarGz),
        ("audio.opus", b"OggS\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0OpusHead", Format::Opus),
    ];
    for (name, bytes, expected) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(detect_format(&path).unwrap(), *expected, "{name}");
    }
    let path = dir.path().join("large.txt");
    std::fs::write(&path, format!("{{\"text\":\"{}\"}}", "ấ".repeat(5000))).unwrap();
    assert_eq!(detect_format(&path).unwrap(), Format::Json);
    let directory = dir.path().join("directory.png");
    std::fs::create_dir(&directory).unwrap();
    assert!(detect_format(directory).is_err());
}

#[test]
fn format_wire_labels_roundtrip() {
    for fmt in [Format::ThreeGp, Format::TarGz, Format::SevenZ] {
        let wire = serde_json::to_string(&fmt).unwrap();
        assert_eq!(wire, format!("\"{}\"", fmt.label()));
        assert_eq!(serde_json::from_str::<Format>(&wire).unwrap(), fmt);
    }
}

#[test]
fn zip_container_detection_reads_entries_not_filename_or_zip_layout() {
    for (entry, content, expected) in [
        ("word/document.xml", "<document/>", Format::Docx),
        ("xl/workbook.xml", "<workbook/>", Format::Xlsx),
        ("ppt/presentation.xml", "<presentation/>", Format::Pptx),
        (
            "mimetype",
            "application/vnd.oasis.opendocument.text",
            Format::Odt,
        ),
        (
            "mimetype",
            "application/vnd.oasis.opendocument.spreadsheet",
            Format::Ods,
        ),
        (
            "mimetype",
            "application/vnd.oasis.opendocument.presentation",
            Format::Odp,
        ),
    ] {
        let dir = tempdir().unwrap();
        let path = dir.path().join("misnamed.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file(
            "unrelated-first-entry",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&vec![0; 9000]).unwrap();
        zip.start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
        zip.finish().unwrap();
        assert_eq!(detect_format(path).unwrap(), expected);
    }
}

#[test]
fn shared_media_signatures_are_disambiguated() {
    let dir = tempdir().unwrap();
    let mut matroska = b"\x1aE\xdf\xa3".to_vec();
    matroska.extend_from_slice(&[0; 20]);
    matroska.extend_from_slice(b"\x42\x82\x88matroska");
    let path = dir.path().join("misnamed.webm");
    std::fs::write(&path, matroska).unwrap();
    assert_eq!(detect_format(&path).unwrap(), Format::Mkv);
    for (name, header, expected) in [
        (
            "audio.m4a",
            &b"\0\0\0\x18ftypisom\0\0\0\0isommp42"[..],
            Format::M4a,
        ),
        (
            "mobile.mp4",
            &b"\0\0\0\x18ftyp3gp5\0\0\0\0isom3gp5"[..],
            Format::ThreeGp,
        ),
        (
            "tagged.flac",
            &b"ID3\x04\0\0\0\0\0\0fLaC\0\0\0\0"[..],
            Format::Flac,
        ),
        (
            "tagged.wav",
            &b"ID3\x04\0\0\0\0\0\0RIFF\0\0\0\0WAVEfmt "[..],
            Format::Wav,
        ),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, header).unwrap();
        assert_eq!(detect_format(&path).unwrap(), expected, "{name}");
    }
}

#[test]
fn path_and_options_boundaries_handle_relative_reserved_and_compound_names() {
    let dir = tempdir().unwrap();
    let opts = Options {
        output_dir: Some(dir.path().to_path_buf()),
        ..Options::default()
    };
    for (source, expected) in [
        ("CON.png", "_CON.jpg"),
        ("archive.tar.gz", "archive.jpg"),
        ("name. .png", "name.jpg"),
    ] {
        let output =
            vertex_core::resolve_target_path(Path::new(source), Format::Jpg, &opts).unwrap();
        assert_eq!(output.file_name().unwrap(), expected);
    }
    assert_eq!(
        vertex_core::path_utils::parent_dir(Path::new("photo.png")),
        Path::new(".")
    );
    let options = Options {
        quality: 0,
        dpi: 0,
        max_width: Some(0),
        output_dir: Some("   ".into()),
        avif_speed: 255,
        video_crf: 255,
        ..Options::default()
    }
    .normalized();
    assert_eq!(
        (
            options.quality,
            options.dpi,
            options.max_width,
            options.output_dir,
            options.avif_speed,
            options.video_crf
        ),
        (1, 72, None, None, 10, 35)
    );
    let file = dir.path().join("ảnh có khoảng trắng.png");
    let url = url::Url::from_file_path(&file).unwrap();
    assert_eq!(
        vertex_core::path_utils::ipc_path(url.as_ref()).unwrap(),
        file
    );
}

#[test]
fn direct_image_conversion_rejects_same_file_and_cleans_cancelled_output() {
    use vertex_core::converters::image::PureRustImageConverter;
    let dir = tempdir().unwrap();
    let input = dir.path().join("source.png");
    image::RgbImage::new(32, 32).save(&input).unwrap();
    let bytes = std::fs::read(&input).unwrap();
    let converter = PureRustImageConverter::default();
    let cancel = CancelToken::new();
    assert!(converter
        .convert(&input, &input, &Options::default(), &|_| {}, &cancel)
        .is_err());
    let linked = dir.path().join("hardlink.png");
    std::fs::hard_link(&input, &linked).unwrap();
    assert!(converter
        .convert(&input, &linked, &Options::default(), &|_| {}, &cancel)
        .is_err());
    let output = dir.path().join("existing.jpg");
    std::fs::write(&output, b"keep existing file").unwrap();
    let result = converter.convert(
        &input,
        &output,
        &Options::default(),
        &|p| {
            if p > 0.5 {
                cancel.cancel();
            }
        },
        &cancel,
    );
    assert!(matches!(result, Err(vertex_core::VertexError::Cancelled)));
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    assert_eq!(std::fs::read(&output).unwrap(), b"keep existing file");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
}

#[test]
fn image_decode_uses_magic_and_applies_exif_orientation() {
    use image::ImageEncoder;
    let dir = tempdir().unwrap();
    let input = dir.path().join("jpeg-with-wrong-extension.png");
    let mut file = std::fs::File::create(&input).unwrap();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut file);
    let exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    encoder.set_exif_metadata(exif.to_vec()).unwrap();
    encoder
        .encode(&[128; 18], 2, 3, image::ExtendedColorType::Rgb8)
        .unwrap();
    drop(file);
    let output = Registry::default()
        .convert(
            &input,
            Format::Png,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap();
    let decoded = image::open(output).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (3, 2));
    let opts = Options {
        strip_metadata: false,
        ..Options::default()
    };
    for target in [Format::Jpg, Format::Png, Format::Webp] {
        use image::ImageDecoder;
        let output = Registry::default()
            .convert(&input, target, &opts, &|_| {}, &CancelToken::new())
            .unwrap();
        let mut decoder = image::ImageReader::open(output)
            .unwrap()
            .into_decoder()
            .unwrap();
        assert!(decoder.exif_metadata().unwrap().is_some());
        assert_eq!(
            decoder.orientation().unwrap(),
            image::metadata::Orientation::NoTransforms
        );
        assert_eq!(decoder.dimensions(), (3, 2));
    }
}

#[test]
fn float_tiff_converts_to_png_and_alpha_resize_does_not_leak_hidden_colors() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("float.tiff");
    image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
        3,
        5,
        image::Rgb([0.25, 0.5, 0.75]),
    ))
    .save(&input)
    .unwrap();
    let registry = Registry::default();
    let output = registry
        .convert(
            &input,
            Format::Png,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(
        image::open(output).unwrap().color(),
        image::ColorType::Rgb16
    );
    let alpha = dir.path().join("alpha.png");
    image::RgbaImage::from_fn(64, 64, |x, _| {
        if x < 32 {
            image::Rgba([255, 0, 0, 0])
        } else {
            image::Rgba([0, 0, 255, 255])
        }
    })
    .save(&alpha)
    .unwrap();
    let opts = Options {
        max_width: Some(16),
        max_height: Some(16),
        ..Options::default()
    };
    let output = registry
        .convert(&alpha, Format::Png, &opts, &|_| {}, &CancelToken::new())
        .unwrap();
    let image = image::open(output).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (16, 16));
    assert!(image.pixels().all(|p| p[0] < 3));
}

#[test]
fn redaction_handles_escaped_quotes_and_unquoted_keys_without_allocating_clean_logs() {
    let input = r#"api_key=private token=hidden secret=classified password="abc\"def" next=public"#;
    let result = SensitiveDataScrubber::scrub(input);
    for secret in ["private", "hidden", "classified", "abc", "def"] {
        assert!(!result.contains(secret));
    }
    assert!(result.contains("next=public"));
    assert!(matches!(
        SensitiveDataScrubber::scrub("ordinary log line"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(!SafeLauncher::is_safe_url("https://"));
    assert!(!SafeLauncher::is_safe_url("https://user:pass@example.com"));
}

#[test]
fn unavailable_engine_does_not_mask_fallback_or_create_output_directories() {
    struct Engine(bool);
    impl Converter for Engine {
        fn name(&self) -> &'static str {
            "test engine"
        }
        fn sources(&self) -> &[Format] {
            &[Format::Png]
        }
        fn targets(&self, _: Format) -> Vec<Format> {
            vec![Format::Jpg]
        }
        fn available(&self) -> Availability {
            if self.0 {
                Availability::Ok
            } else {
                Availability::Missing("test engine absent".into())
            }
        }
        fn convert(
            &self,
            _: &Path,
            output: &Path,
            _: &Options,
            _: &dyn Fn(f32),
            _: &CancelToken,
        ) -> vertex_core::Result<()> {
            std::fs::write(output, b"completed").unwrap();
            Ok(())
        }
    }
    let dir = tempdir().unwrap();
    let input = dir.path().join("source.png");
    image::RgbImage::new(2, 2).save(&input).unwrap();
    let opts = Options {
        output_dir: Some(dir.path().join("new-output")),
        collision_policy: CollisionPolicy::FailIfExists,
        ..Options::default()
    };
    let mut registry = Registry::new();
    registry.register(std::sync::Arc::new(Engine(false)));
    assert!(matches!(
        registry.convert(&input, Format::Jpg, &opts, &|_| {}, &CancelToken::new()),
        Err(vertex_core::VertexError::EngineMissing(_))
    ));
    assert!(!opts.output_dir.as_ref().unwrap().exists());
    registry.register(std::sync::Arc::new(Engine(true)));
    assert!(registry
        .convert(&input, Format::Jpg, &opts, &|_| {}, &CancelToken::new())
        .is_ok());
}

#[cfg(windows)]
#[test]
fn explorer_accepts_extended_local_paths_and_origin_hints() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("local file.png");
    std::fs::write(&input, b"data").unwrap();
    assert!(SafeLauncher::sanitize_explorer_path(&input.canonicalize().unwrap()).is_ok());
    let content = "[ZoneTransfer]\r\n hosturl = about:internet\r\n REFERRERURL = https://github.com/rainaku/Vertex/releases";
    let (host, referrer) = vertex_core::AppIntegrityService::parse_zone_identifier(content);
    assert_eq!(host.as_deref(), Some("about:internet"));
    assert!(referrer.unwrap().starts_with("https://github.com/"));
    let mut ads = input.as_os_str().to_os_string();
    ads.push(":Zone.Identifier");
    std::fs::write(ads, content).unwrap();
    assert!(vertex_core::AppIntegrityService::check_download_origin(&input).0);
}

#[test]
fn concurrent_same_stem_conversions_publish_unique_complete_files() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("photo.png");
    image::RgbImage::new(32, 32).save(&input).unwrap();
    let registry = Registry::default();
    let barrier = std::sync::Barrier::new(4);
    let outputs = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let registry = &registry;
                let input = &input;
                let barrier = &barrier;
                scope.spawn(move || {
                    registry
                        .convert(
                            input,
                            Format::Jpg,
                            &Options::default(),
                            &|p| {
                                if p == 0.1 {
                                    barrier.wait();
                                }
                            },
                            &CancelToken::new(),
                        )
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        outputs
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    for output in outputs {
        assert!(image::open(output).is_ok());
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 5);
}
