use std::fs;
use vertex_core::converters::{
    data::DataConverter, image::PureRustImageConverter, render::SvgConverter,
};
use vertex_core::{CancelToken, Converter, Options};

#[test]
fn direct_table_transcoding_matches_the_json_route_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    for (extension, text) in [
        (
            "csv",
            "z,a,m\nTiếng Việt,0001,\"comma, tab\tline\n😀\"\nlast,,true\n",
        ),
        (
            "tsv",
            "z\ta\tm\nTiếng Việt\t0001\t\"comma, tab\tline\n😀\"\nlast\t\ttrue\n",
        ),
        ("csv", "\u{feff}z,a\n1,2\n"),
        ("csv", "z,a\n"),
        ("csv", ""),
        ("csv", "\"\",a\nempty,001\n"),
    ] {
        let input = dir.path().join(format!("input.{extension}"));
        fs::write(&input, text).unwrap();
        for target in ["csv", "tsv"] {
            let direct = dir.path().join(format!("direct.{target}"));
            let json = dir.path().join("middle.json");
            let indirect = dir.path().join(format!("indirect.{target}"));
            for (from, to) in [(&input, &direct), (&input, &json), (&json, &indirect)] {
                DataConverter
                    .convert(from, to, &Options::default(), &|_| {}, &cancel)
                    .unwrap();
            }
            assert_eq!(
                fs::read(direct).unwrap(),
                fs::read(indirect).unwrap(),
                "{extension} -> {target}: {text:?}"
            );
        }
    }
}

#[test]
fn fast_table_errors_and_cancellation_preserve_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.csv");
    let output = dir.path().join("output.tsv");
    fs::write(&output, b"original").unwrap();
    for text in [
        b"a,a\n1,2\n".as_slice(),
        b"a,b\n1\n",
        b"a,b\n1,2,3\n",
        b"a,b\n\xff,2\n",
    ] {
        fs::write(&input, text).unwrap();
        assert!(DataConverter
            .convert(
                &input,
                &output,
                &Options::default(),
                &|_| {},
                &CancelToken::new()
            )
            .is_err());
        assert_eq!(fs::read(&output).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    fs::write(&input, "a,b\n1,2\n").unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(DataConverter
        .convert(&input, &output, &Options::default(), &|_| {}, &cancel)
        .is_err());
    assert_eq!(fs::read(output).unwrap(), b"original");
}

#[test]
fn svg_in_memory_pipeline_matches_png_intermediate_for_every_encoder() {
    let dir = tempfile::tempdir().unwrap();
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="33" height="17"><defs><linearGradient id="g"><stop stop-color="#f93" stop-opacity="0"/><stop offset="1" stop-color="#37f" stop-opacity="0.8"/></linearGradient></defs><rect width="33" height="17" fill="url(#g)"/><circle cx="16" cy="8" r="5" fill="#3a7" opacity="0.4"/></svg>"##;
    let input = dir.path().join("input.svg");
    fs::write(&input, svg).unwrap();
    let tree = resvg::usvg::Tree::from_data(svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(33, 17).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let intermediate = dir.path().join("legacy.png");
    pixmap.save_png(&intermediate).unwrap();
    let cancel = CancelToken::new();
    for extension in ["png", "jpg", "webp", "avif", "bmp", "ico", "tiff", "gif"] {
        let direct = dir.path().join(format!("direct.{extension}"));
        let legacy = dir.path().join(format!("reference.{extension}"));
        let options = Options {
            jpeg_background: [23, 45, 67],
            gif_alpha_threshold: 90,
            avif_speed: 10,
            ..Default::default()
        };
        SvgConverter
            .convert(&input, &direct, &options, &|_| {}, &cancel)
            .unwrap();
        PureRustImageConverter
            .convert(&intermediate, &legacy, &options, &|_| {}, &cancel)
            .unwrap();
        assert_eq!(
            fs::read(direct).unwrap(),
            fs::read(legacy).unwrap(),
            "{extension}"
        );
    }
}
