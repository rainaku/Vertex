use std::fs;
use std::io::Read;
use std::path::Path;
use tempfile::tempdir;
use vertex_core::{CancelToken, Converter, Format, Options, Registry};

fn document_xml(path: &Path) -> String {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn editable_pdf() -> Vec<u8> {
    let first = "BT /F2 18 Tf 0 0 1 rg 40 350 Td (Editable heading) Tj ET\n\
        0 0 0 RG 1 w 40 220 200 60 re S 40 250 m 240 250 l S 140 220 m 140 280 l S\n\
        BT /F1 12 Tf 0 0 0 rg 50 260 Td (Name) Tj 100 0 Td (Value) Tj ET\n\
        BT /F1 12 Tf 50 230 Td (Alpha) Tj 100 0 Td (42) Tj ET\n";
    let second = "BT /F1 14 Tf 40 250 Td (Second page) Tj ET\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 7 0 R /F2 8 0 R >> >> /Contents 4 0 R >>".into(),
        format!("<< /Length {} >>\nstream\n{first}endstream", first.len()),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>".into(),
        format!("<< /Length {} >>\nstream\n{second}endstream", second.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".into(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn legacy_word_and_editable_pdf_routes_are_exposed_with_fidelity_notices() {
    let definitions = Registry::default().source_formats();
    let doc = definitions
        .iter()
        .find(|d| d.format == Format::Doc)
        .unwrap();
    assert_eq!(Format::from_mime("application/msword"), Some(Format::Doc));
    assert_eq!(Format::from_extension(".DOC"), Some(Format::Doc));
    assert!(doc.targets.contains(&Format::Pdf));
    assert!(doc.targets.contains(&Format::Docx));
    let pdf = definitions
        .iter()
        .find(|d| d.format == Format::Pdf)
        .unwrap();
    assert!(pdf.targets.contains(&Format::Docx));
    assert!(pdf.notices["DOCX"].en.contains("Editable"));
    assert!(pdf.notices["DOCX"].en.contains("OCR"));
    assert!(doc.notices["TXT"].en.contains("plain text only"));
}

#[test]
fn utf16_text_to_html_preserves_unicode_tabs_and_leading_blank_lines() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("tiếng Việt.txt");
    let text = "\nTiếng Việt 😀\t  <nội dung>\n\n";
    let mut bytes = vec![0xff, 0xfe];
    for word in text.encode_utf16() {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    fs::write(&input, bytes).unwrap();
    let output = Registry::default()
        .convert(
            &input,
            Format::Html,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap();
    let html = fs::read_to_string(output).unwrap();
    assert!(html.contains("<pre>\n\nTiếng Việt 😀\t  &lt;nội dung&gt;\n\n</pre>"));
}

#[test]
fn invalid_text_encoding_does_not_replace_an_existing_output() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.txt");
    let output = dir.path().join("bad.html");
    fs::write(&input, [0xff, 0xfe, 0x41]).unwrap();
    fs::write(&output, "original").unwrap();
    let options = Options {
        collision_policy: vertex_core::CollisionPolicy::Overwrite,
        ..Default::default()
    };
    assert!(Registry::default()
        .convert(&input, Format::Html, &options, &|_| {}, &CancelToken::new())
        .is_err());
    assert_eq!(fs::read_to_string(output).unwrap(), "original");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn real_pdf_to_word_preserves_editable_text_tables_and_multiple_pages() {
    let converter = vertex_core::converters::pdf_document::PdfDocumentConverter;
    if !converter.available().is_ok() {
        eprintln!("Skipping PDF → DOCX integration: pdf2docx is not installed");
        return;
    }
    let dir = tempdir().unwrap();
    let input = dir.path().join("tài liệu.pdf");
    fs::write(&input, editable_pdf()).unwrap();
    let output = Registry::default()
        .convert(
            &input,
            Format::Docx,
            &Options::default(),
            &|_| {},
            &CancelToken::new(),
        )
        .unwrap();
    let xml = document_xml(&output);
    for text in [
        "Editable heading",
        "Name",
        "Value",
        "Alpha",
        "42",
        "Second page",
    ] {
        assert!(xml.contains(text), "Missing editable text: {text}");
    }
    assert!(
        xml.contains("<w:tbl>"),
        "The table must remain an editable Word table"
    );
    assert!(
        xml.contains("<w:b"),
        "Bold heading formatting must be retained"
    );
    assert!(
        xml.contains("0000FF"),
        "Blue heading formatting must be retained"
    );
    assert!(xml.matches("<w:sectPr").count() >= 2);
    assert!(xml.contains("w:w=\"6000\" w:h=\"8000\""));
    assert!(xml.contains("w:w=\"8000\" w:h=\"6000\""));

    // Cancellation after reconstruction must still leave the old output intact.
    let options = Options {
        collision_policy: vertex_core::CollisionPolicy::Overwrite,
        ..Default::default()
    };
    fs::write(&output, "original").unwrap();
    let cancel = CancelToken::new();
    let result = Registry::default().convert(
        &input,
        Format::Docx,
        &options,
        &|p| {
            if p >= 0.95 {
                cancel.cancel();
            }
        },
        &cancel,
    );
    assert!(matches!(result, Err(vertex_core::VertexError::Cancelled)));
    assert_eq!(fs::read_to_string(&output).unwrap(), "original");
    fs::write(&input, b"%PDF-1.4\nbroken").unwrap();
    assert!(Registry::default()
        .convert(&input, Format::Docx, &options, &|_| {}, &CancelToken::new())
        .is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "original");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
