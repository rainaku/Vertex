use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use vertex_core::{detect_format, CancelToken, Category, Converter, Format, Options, Registry};

const NEW_DOCUMENTS: &[Format] = &[
    Format::Docm,
    Format::Dot,
    Format::Dotx,
    Format::Dotm,
    Format::Ott,
    Format::Fodt,
    Format::Sxw,
    Format::Wps,
    Format::Wpd,
    Format::Wri,
];

fn zip_file(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, content) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn office_variants_are_detected_by_package_type_even_with_misleading_extensions() {
    let dir = tempfile::tempdir().unwrap();
    for (format, mime) in [
        (
            Format::Docx,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        (
            Format::Docm,
            "application/vnd.ms-word.document.macroEnabled.main+xml",
        ),
        (
            Format::Dotx,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
        ),
        (
            Format::Dotm,
            "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
        ),
    ] {
        let types = format!(
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="{mime}"/></Types>"#
        );
        // DOCX content in a DOCM-named file must still be identified as DOCX.
        for suffix in ["bin", "docm"] {
            let path = dir.path().join(format!("renamed.{suffix}"));
            zip_file(
                &path,
                &[
                    ("word/document.xml", "<document/>"),
                    ("[Content_Types].xml", &types),
                ],
            );
            assert_eq!(detect_format(&path).unwrap(), format);
        }
    }
    for (format, mime) in [
        (
            Format::Ott,
            "application/vnd.oasis.opendocument.text-template",
        ),
        (Format::Sxw, "application/vnd.sun.xml.writer"),
    ] {
        let path = dir.path().join("renamed.zip");
        zip_file(&path, &[("mimetype", mime), ("content.xml", "<document/>")]);
        assert_eq!(detect_format(path).unwrap(), format);
    }
}

#[test]
fn flat_odf_is_not_misrouted_to_the_structured_data_xml_converter() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("document.xml");
    fs::write(&path, r#"<?xml version="1.0"?><o:document xmlns:o="urn:oasis:names:tc:opendocument:xmlns:office:1.0" o:mimetype="application/vnd.oasis.opendocument.text"><o:body/></o:document>"#).unwrap();
    assert_eq!(detect_format(&path).unwrap(), Format::Fodt);
    fs::write(
        &path,
        "<document mimetype=\"application/vnd.oasis.opendocument.text\"/>",
    )
    .unwrap();
    assert_eq!(detect_format(&path).unwrap(), Format::Xml);
}

#[test]
fn new_inputs_expose_real_document_routes_and_macro_limitations() {
    let registry = Registry::default();
    let definitions = registry.source_formats();
    for format in NEW_DOCUMENTS {
        assert_eq!(format.category(), Category::Document);
        assert_eq!(
            Format::from_extension(&format.extension().to_uppercase()),
            Some(*format)
        );
        let definition = definitions.iter().find(|d| d.format == *format).unwrap();
        for target in [Format::Pdf, Format::Docx, Format::Txt] {
            assert!(definition.targets.contains(&target), "{format} → {target}");
        }
        assert!(!definition.targets.contains(format));
        for input_only in [
            Format::Docm,
            Format::Dot,
            Format::Dotm,
            Format::Sxw,
            Format::Wps,
            Format::Wpd,
            Format::Wri,
        ] {
            assert!(!definition.targets.contains(&input_only));
        }
    }
    for format in [Format::Docm, Format::Dotm] {
        let definition = definitions.iter().find(|d| d.format == format).unwrap();
        assert!(definition.notices["DOCX"]
            .en
            .contains("macros are disabled"));
    }
    let docx = definitions
        .iter()
        .find(|d| d.format == Format::Docx)
        .unwrap();
    for target in [Format::Dotx, Format::Ott, Format::Fodt] {
        assert!(docx.targets.contains(&target));
    }
}

#[test]
fn text_and_markup_aliases_use_existing_unicode_converters() {
    let registry = Registry::default();
    let dir = tempfile::tempdir().unwrap();
    for suffix in ["nfo", "asc", "mdown", "mkd", "mdwn", "xhtml", "xht"] {
        let path = dir.path().join(format!("tiếng Việt.{suffix}"));
        let (format, content) = if ["xhtml", "xht"].contains(&suffix) {
            (
                Format::Html,
                "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>Tiếng Việt</body></html>",
            )
        } else if ["nfo", "asc"].contains(&suffix) {
            (Format::Txt, "Tiếng Việt\t  <nội dung>")
        } else {
            (Format::Md, "# Tiếng Việt\n\n**đậm**")
        };
        fs::write(&path, content).unwrap();
        assert_eq!(detect_format(&path).unwrap(), format);
        if format != Format::Html {
            let output = registry
                .convert(
                    &path,
                    Format::Html,
                    &Options::default(),
                    &|_| {},
                    &CancelToken::new(),
                )
                .unwrap();
            let html = fs::read_to_string(output).unwrap();
            assert!(html.contains("Tiếng Việt"));
            assert!(html.contains(if format == Format::Md {
                "<strong>đậm</strong>"
            } else {
                "&lt;nội dung&gt;"
            }));
        }
    }
}

#[test]
fn real_flat_odf_and_templates_keep_editable_content_when_libreoffice_is_installed() {
    if !vertex_core::converters::office::OfficeConverter
        .available()
        .is_ok()
    {
        eprintln!("Skipping extended Office integration: LibreOffice is not installed");
        return;
    }
    let registry = Registry::default();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tài liệu.fodt");
    fs::write(&input, r#"<?xml version="1.0" encoding="UTF-8"?><office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.2" office:mimetype="application/vnd.oasis.opendocument.text"><office:automatic-styles><style:style style:name="Bold" style:family="paragraph"><style:text-properties fo:font-weight="bold"/></style:style></office:automatic-styles><office:body><office:text><text:p text:style-name="Bold">Tiếng Việt giữ định dạng</text:p><text:p>Dòng thứ hai</text:p></office:text></office:body></office:document>"#).unwrap();
    let source_word = registry
        .convert(
            &input,
            Format::Docx,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap();
    for target in [Format::Dotx, Format::Ott, Format::Fodt] {
        let output = registry
            .convert(
                &source_word,
                target,
                &Options::default(),
                &|_| {},
                &CancelToken::new(),
            )
            .unwrap();
        assert_eq!(detect_format(&output).unwrap(), target);
        let word = registry
            .convert(
                &output,
                Format::Docx,
                &Options::default(),
                &|_| {},
                &CancelToken::new(),
            )
            .unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(word).unwrap()).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("Tiếng Việt giữ định dạng"));
        assert!(xml.contains("Dòng thứ hai"));
        if let Ok(mut styles) = zip.by_name("word/styles.xml") {
            styles.read_to_string(&mut xml).unwrap();
        }
        assert!(xml.contains("<w:b"));
    }
}
