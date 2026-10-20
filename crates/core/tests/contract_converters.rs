use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use tempfile::tempdir;
use vertex_core::{detect_format, Availability, CancelToken, Converter, Format, Options, Registry};

fn convert(registry: &Registry, input: &Path, target: Format) -> std::path::PathBuf {
    registry
        .convert(
            input,
            target,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap()
}

#[test]
fn frontend_contract_matches_registry_and_every_alias_has_a_converter() {
    let registry = Registry::default();
    let snapshot: serde_json::Value = serde_json::from_str(include_str!(
        "../../../frontend/src/lib/conversion-contract.json"
    ))
    .unwrap();
    assert_eq!(
        snapshot,
        serde_json::to_value(registry.contract()).unwrap(),
        "Run cargo run -p vertex-core --example export_contract"
    );
    let definitions = registry.source_formats();
    assert_eq!(definitions.len(), Format::ALL.len());
    let mut extensions = std::collections::HashSet::new();
    for definition in definitions {
        assert!(!definition.targets.is_empty(), "{}", definition.format);
        for extension in definition.extensions {
            assert!(extensions.insert(extension));
            assert_eq!(Format::from_extension(extension), Some(definition.format));
        }
        assert_eq!(
            serde_json::to_value(definition.format).unwrap(),
            definition.label
        );
    }
}

#[test]
fn structured_data_roundtrips_preserve_types_and_unicode() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("dữ liệu.json");
    let value = serde_json::json!({"title":"Tiếng Việt", "enabled":true, "nested":{"count":42,"ratio":1.5,"items":[1,2,3]}});
    fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    for target in [Format::Yaml, Format::Toml] {
        let encoded = convert(&registry, &input, target);
        let decoded = convert(&registry, &encoded, Format::Json);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(decoded).unwrap()).unwrap(),
            value
        );
    }
}

#[test]
fn tables_preserve_quoted_cells_delimiters_and_newlines() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("table.csv");
    fs::write(
        &input,
        "name,note\r\nTiếng Việt,\"comma, tab\tand\nnewline\"\r\n",
    )
    .unwrap();
    let json = convert(&registry, &input, Format::Json);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
    assert_eq!(value[0]["note"], "comma, tab\tand\nnewline");
    let tsv = convert(&registry, &json, Format::Tsv);
    let decoded = convert(&registry, &tsv, Format::Json);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(decoded).unwrap()).unwrap(),
        value
    );
}

#[test]
fn data_errors_do_not_publish_partial_files_or_overwrite_existing_output() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.json");
    fs::write(&input, r#"[{"nested":{"x":1}}]"#).unwrap();
    let output = dir.path().join("bad.csv");
    fs::write(&output, b"original").unwrap();
    let options = Options {
        collision_policy: vertex_core::CollisionPolicy::Overwrite,
        ..Default::default()
    };
    assert!(registry
        .convert(&input, Format::Csv, &options, &|_| {}, &CancelToken::new())
        .is_err());
    assert_eq!(fs::read(&output).unwrap(), b"original");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    let duplicate = dir.path().join("duplicate.csv");
    fs::write(&duplicate, "a,a\n1,2\n").unwrap();
    assert!(registry
        .convert(
            &duplicate,
            Format::Json,
            &Options::default(),
            &|_| {},
            &CancelToken::new()
        )
        .is_err());
}

#[test]
fn xml_mapping_preserves_attributes_repeated_elements_and_entities() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("data.xml");
    fs::write(&input, "<catalog lang=\"vi\"><name>A &amp; B</name><item>một</item><item>hai</item><empty/></catalog>").unwrap();
    let json = convert(&registry, &input, Format::Json);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
    assert_eq!(value["catalog"]["@lang"], "vi");
    assert_eq!(value["catalog"]["name"], "A & B");
    assert_eq!(value["catalog"]["item"], serde_json::json!(["một", "hai"]));
    let xml = convert(&registry, &json, Format::Xml);
    let decoded = convert(&registry, &xml, Format::Json);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(decoded).unwrap()).unwrap(),
        value
    );
    for bad in [
        "<r/><r/>",
        "<!DOCTYPE r SYSTEM 'file:///secret'><r/>",
        "<r>before<x/>after</r>",
        "<r><x></r>",
    ] {
        fs::write(&input, bad).unwrap();
        assert!(
            registry
                .convert(
                    &input,
                    Format::Json,
                    &Options::default(),
                    &|_| {},
                    &CancelToken::new()
                )
                .is_err(),
            "{bad}"
        );
    }
}

#[test]
fn markdown_and_plain_text_have_real_utf8_html_converters() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let md = dir.path().join("readme.md");
    fs::write(&md, "# Tiếng Việt\n\n**đậm** & nội dung").unwrap();
    let html = convert(&registry, &md, Format::Html);
    let text = fs::read_to_string(html).unwrap();
    assert!(text.contains("<h1>Tiếng Việt</h1>"));
    assert!(text.contains("<strong>đậm</strong>"));
    let txt = dir.path().join("plain.txt");
    fs::write(&txt, "Literal: <script> & Tiếng Việt").unwrap();
    assert!(fs::read_to_string(convert(&registry, &txt, Format::Html))
        .unwrap()
        .contains("&lt;script&gt; &amp; Tiếng Việt"));
}

#[test]
fn svg_dpi_dimensions_and_jpeg_background_are_applied() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let svg = dir.path().join("vector.svg");
    fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="0.5in"><rect width="50%" height="100%" fill="red"/></svg>"#).unwrap();
    for dpi in [72, 200, 600] {
        let opts = Options {
            dpi,
            ..Default::default()
        };
        let output = registry
            .convert(&svg, Format::Png, &opts, &|_| {}, &CancelToken::new())
            .unwrap();
        assert_eq!(image::image_dimensions(output).unwrap(), (dpi, dpi / 2));
    }
    let opts = Options {
        dpi: 600,
        max_width: Some(80),
        jpeg_background: [0, 0, 255],
        ..Default::default()
    };
    let output = registry
        .convert(&svg, Format::Jpg, &opts, &|_| {}, &CancelToken::new())
        .unwrap();
    let img = image::open(output).unwrap().into_rgb8();
    assert_eq!(img.dimensions(), (80, 40));
    assert!(img.get_pixel(70, 20)[2] > 240);
}

fn make_zip(path: &Path) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.add_directory("empty/", opts).unwrap();
    zip.start_file("nested/tiếng Việt.txt", opts).unwrap();
    zip.write_all(b"archive contents\0\xff").unwrap();
    zip.finish().unwrap();
}

#[test]
fn archives_roundtrip_zip_tar_gz_and_7z_without_losing_names_or_bytes() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("sample.zip");
    make_zip(&input);
    for target in [Format::TarGz, Format::SevenZ] {
        let archive = convert(&registry, &input, target);
        assert_eq!(detect_format(&archive).unwrap(), target);
        let output = convert(&registry, &archive, Format::Zip);
        let mut zip = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
        assert!(zip.by_name("empty/").unwrap().is_dir());
        let mut bytes = Vec::new();
        zip.by_name("nested/tiếng Việt.txt")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"archive contents\0\xff");
    }
}

#[test]
fn gzip_payload_is_a_real_input_format_and_compound_tar_names_are_preserved() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("sample.txt.gz");
    let mut gzip = flate2::write::GzEncoder::new(
        fs::File::create(&input).unwrap(),
        flate2::Compression::default(),
    );
    gzip.write_all(b"content").unwrap();
    gzip.finish().unwrap();
    assert_eq!(detect_format(&input).unwrap(), Format::Gz);
    let output = convert(&registry, &input, Format::Zip);
    let mut zip = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
    assert_eq!(zip.by_index(0).unwrap().name(), "sample.txt");
    let input = dir.path().join("sample.zip");
    make_zip(&input);
    let tar = convert(&registry, &input, Format::TarGz);
    let renamed = dir.path().join("disguised.gz");
    fs::copy(tar, &renamed).unwrap();
    assert_eq!(detect_format(renamed).unwrap(), Format::TarGz);
}

#[test]
fn archive_traversal_links_and_case_collisions_are_rejected() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    for names in [
        vec!["../escape.txt"],
        vec!["C:/escape.txt"],
        vec!["folder/CON.txt"],
        vec!["A.txt", "a.txt"],
    ] {
        let input = dir.path().join("bad.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&input).unwrap());
        for name in names {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"bad").unwrap();
        }
        zip.finish().unwrap();
        assert!(registry
            .convert(
                &input,
                Format::TarGz,
                &Options::default(),
                &|_| {},
                &CancelToken::new()
            )
            .is_err());
        assert!(!dir.path().join("bad.tar.gz").exists());
    }
}

#[test]
fn cancellation_cleans_up_new_converters() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("sample.zip");
    make_zip(&input);
    let cancel = CancelToken::new();
    let result = registry.convert(
        &input,
        Format::SevenZ,
        &Options::default(),
        &|_| cancel.cancel(),
        &cancel,
    );
    assert!(matches!(result, Err(vertex_core::VertexError::Cancelled)));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn optional_engines_report_missing_dependencies_instead_of_empty_target_lists() {
    let registry = Registry::default();
    for from in [
        Format::Heic,
        Format::Avif,
        Format::Pdf,
        Format::Docx,
        Format::Xlsx,
        Format::Pptx,
    ] {
        let targets = registry.available_targets(from);
        assert!(!targets.is_empty(), "{from}");
        for (_, availability) in targets {
            if let Availability::Missing(reason) = availability {
                assert!(!reason.is_empty());
            }
        }
    }
}

#[test]
fn real_avif_decoder_roundtrips_when_ffmpeg_is_installed() {
    if vertex_core::converters::ffmpeg::find_ffmpeg().is_none() {
        return;
    }
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("source.png");
    image::RgbImage::from_pixel(16, 12, image::Rgb([250, 20, 10]))
        .save(&input)
        .unwrap();
    let avif = convert(&registry, &input, Format::Avif);
    let png = convert(&registry, &avif, Format::Png);
    assert_eq!(image::image_dimensions(png).unwrap(), (16, 12));
}

fn simple_pdf() -> Vec<u8> {
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for object in [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 36] /Contents 4 0 R >>",
        "<< /Length 24 >>\nstream\n1 0 0 rg 0 0 72 36 re f\nendstream",
    ] {
        offsets.push(pdf.len());
        pdf.extend_from_slice(
            format!("{} 0 obj\n{object}\nendobj\n", offsets.len() - 1).as_bytes(),
        );
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in &offsets[1..] {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

#[test]
fn real_pdf_renderer_uses_dpi_and_raster_options_when_poppler_is_installed() {
    let converter = vertex_core::converters::render::PdfConverter;
    if !converter.available().is_ok() {
        return;
    }
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    let input = dir.path().join("document.pdf");
    fs::write(&input, simple_pdf()).unwrap();
    for dpi in [72, 144] {
        let options = Options {
            dpi,
            ..Default::default()
        };
        let output = registry
            .convert(&input, Format::Png, &options, &|_| {}, &CancelToken::new())
            .unwrap();
        assert_eq!(image::image_dimensions(output).unwrap(), (dpi, dpi / 2));
    }
}

#[test]
fn ipc_output_directories_decode_file_urls_and_reject_nul() {
    let directory = std::path::absolute("dữ liệu output").unwrap();
    let url = url::Url::from_directory_path(&directory).unwrap();
    assert_eq!(
        vertex_core::path_utils::ipc_path(url.as_str()).unwrap(),
        directory
    );
    assert!(vertex_core::path_utils::ipc_path("file:///C:/bad%00dir").is_err());
    assert!(vertex_core::path_utils::ipc_path("bad\0dir").is_err());
}

#[test]
fn data_mapping_rejects_unrepresentable_values_instead_of_silently_changing_them() {
    let registry = Registry::default();
    let dir = tempdir().unwrap();
    for (name, text, target) in [
        ("bad.yaml", "value: .nan", Format::Json),
        ("bad.yaml", "true: yes\n1: value", Format::Json),
        ("bad.toml", "value = nan", Format::Json),
        ("bad.json", r#"{"value":"\u0000"}"#, Format::Xml),
        ("bad.json", r#"{"value":null}"#, Format::Toml),
    ] {
        let input = dir.path().join(name);
        fs::write(&input, text).unwrap();
        assert!(
            registry
                .convert(
                    &input,
                    target,
                    &Options::default(),
                    &|_| {},
                    &CancelToken::new()
                )
                .is_err(),
            "{name}: {text}"
        );
    }
}
