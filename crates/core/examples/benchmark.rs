//! Reproducible end-to-end workloads. Run in release mode; fixture creation and
//! output validation/hashing are excluded from the timed region.
use sha2::{Digest, Sha256};
use std::{fs, hint::black_box, io::Write, time::Instant};
use vertex_core::{CancelToken, CollisionPolicy, Format, Options, Registry};

fn main() {
    let samples: usize = std::env::var("VERTEX_BENCH_SAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    assert!(samples >= 3);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let svg = root.join("vector.svg");
    fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080"><defs><linearGradient id="g"><stop stop-color="#f93" stop-opacity="0.2"/><stop offset="1" stop-color="#37f"/></linearGradient></defs><rect width="1920" height="1080" fill="url(#g)"/><circle cx="960" cy="540" r="400" fill="#3a7" opacity="0.6"/><text x="100" y="160" font-size="72">Vertex Tiếng Việt</text></svg>"##).unwrap();
    let png = root.join("photo.png");
    image::RgbaImage::from_fn(1920, 1080, |x, y| {
        image::Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            ((x * 7 + y * 3) % 256) as u8,
            ((x + y) % 256) as u8,
        ])
    })
    .save(&png)
    .unwrap();
    let csv = root.join("table.csv");
    let mut table = String::from("z_name,a_id,m_note\n");
    for n in 0..50_000 {
        table.push_str(&format!(
            "Tiếng Việt {n},{n},\"comma, tab\tand newline\nrow {n}\"\n"
        ));
    }
    fs::write(&csv, &table).unwrap();
    let txt = root.join("text.txt");
    fs::write(&txt, "Tiếng Việt 😀 <tag> & text\r\n".repeat(150_000)).unwrap();
    let md = root.join("document.md");
    fs::write(
        &md,
        "# Vertex\n\n**Tiếng Việt** and [link](https://example.com).\n\n".repeat(10_000),
    )
    .unwrap();
    let zip = root.join("archive.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&zip).unwrap());
    for n in 0..32 {
        writer
            .start_file(
                format!("folder/{n}.txt"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(&table.as_bytes()[..64 * 1024]).unwrap();
    }
    writer.finish().unwrap();
    let registry = Registry::default();
    let cancel = CancelToken::new();
    let defaults = Options {
        collision_policy: CollisionPolicy::Overwrite,
        output_dir: Some(root.join("out")),
        ..Default::default()
    };
    fs::create_dir(root.join("out")).unwrap();
    let json = registry
        .convert(&csv, Format::Json, &defaults, &|_| {}, &cancel)
        .unwrap();
    let resized = Options {
        max_width: Some(640),
        max_height: Some(360),
        ..defaults.clone()
    };
    let cases = [
        ("svg_png_1080p", &svg, Format::Png, &defaults),
        ("svg_jpeg_1080p", &svg, Format::Jpg, &defaults),
        ("png_jpeg_1080p", &png, Format::Jpg, &defaults),
        ("png_resize_webp_640", &png, Format::Webp, &resized),
        ("csv_tsv_50k", &csv, Format::Tsv, &defaults),
        ("csv_json_50k", &csv, Format::Json, &defaults),
        ("json_csv_50k", &json, Format::Csv, &defaults),
        ("text_html_150k", &txt, Format::Html, &defaults),
        ("markdown_html_10k", &md, Format::Html, &defaults),
        ("zip_targz_32_files", &zip, Format::TarGz, &defaults),
    ];
    let mut results = Vec::new();
    for (name, input, target, options) in cases {
        // Warm up font discovery, allocator, codec tables and OS file cache.
        for _ in 0..2 {
            registry
                .convert(input, target, options, &|_| {}, &cancel)
                .unwrap();
        }
        let mut times = Vec::new();
        let mut output = Default::default();
        for _ in 0..samples {
            let start = Instant::now();
            output = black_box(
                registry
                    .convert(
                        black_box(input),
                        target,
                        options,
                        &|p| {
                            black_box(p);
                        },
                        &cancel,
                    )
                    .unwrap(),
            );
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let bytes = fs::read(&output).unwrap();
        let pixel_sha256 = if target.category() == vertex_core::Category::Image {
            let decoded = image::open(&output).unwrap().into_rgba8();
            Some(format!("{:x}", Sha256::digest(decoded.as_raw())))
        } else {
            None
        };
        let mut sorted = times.clone();
        sorted.sort_by(f64::total_cmp);
        eprintln!("{name}: {:.3} ms", sorted[samples / 2]);
        results.push(serde_json::json!({
            "name": name, "samples_ms": times, "median_ms": sorted[samples / 2],
            "min_ms": sorted[0], "max_ms": sorted[samples - 1],
            "input_bytes": fs::metadata(input).unwrap().len(), "output_bytes": bytes.len(),
            "sha256": format!("{:x}", Sha256::digest(&bytes)), "pixel_sha256": pixel_sha256
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "samples": samples, "warmups": 2, "workloads": results
        }))
        .unwrap()
    );
}
