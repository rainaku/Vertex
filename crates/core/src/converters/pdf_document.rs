//! Reconstruct editable paragraphs and tables, rather than rasterizing PDF pages.
use super::external::{copy_output, find_tool, io_error, run_tool, workspace};
use crate::{Availability, CancelToken, Converter, Format, Options, Result, VertexError};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub struct PdfDocumentConverter;

fn binary() -> Option<PathBuf> {
    find_tool(
        "pdf2docx",
        &["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"],
    )
    .or_else(|| {
        // The explicit setup script installs into a user-owned virtualenv.
        // Resolve it at runtime so a running app sees installation immediately,
        // without inheriting a new PATH or depending on a development checkout.
        #[cfg(windows)]
        {
            let base = std::env::var_os("USERPROFILE")?;
            let path = PathBuf::from(base).join(".vertex/tools/pdf2docx/Scripts/pdf2docx.exe");
            path.is_file().then_some(path)
        }
        #[cfg(not(windows))]
        None
    })
}

fn invalid(message: impl Into<String>) -> VertexError {
    VertexError::Encoding(message.into())
}

impl Converter for PdfDocumentConverter {
    fn name(&self) -> &'static str {
        "PdfDocumentConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Pdf]
    }
    fn targets(&self, from: Format) -> Vec<Format> {
        if from == Format::Pdf {
            vec![Format::Docx]
        } else {
            Vec::new()
        }
    }
    fn available(&self) -> Availability {
        if binary().is_some() {
            Availability::Ok
        } else {
            Availability::Missing("Thiếu pdf2docx để chuyển PDF sang Word. Cài công cụ theo hướng dẫn của Vertex. / PDF to Word requires pdf2docx. Follow Vertex's tool setup instructions.".into())
        }
    }
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input, output)?;
        if Format::from_path_extension(output) != Some(Format::Docx) {
            return Err(invalid("Editable PDF export requires a DOCX target"));
        }
        let binary = binary().ok_or_else(|| VertexError::EngineMissing("pdf2docx".into()))?;
        let temp = workspace()?;
        // Controlled filenames avoid CLI option parsing and keep all side effects
        // in the job directory, including when the source path contains Unicode.
        let source = temp.path().join("source.pdf");
        let generated = temp.path().join("output.docx");
        std::fs::copy(input, &source).map_err(|e| io_error(input, e))?;
        progress(0.05);
        run_tool(
            Command::new(binary).current_dir(temp.path()).args([
                "convert",
                "source.pdf",
                "output.docx",
                // Defaults otherwise allow failed pages to be silently skipped,
                // and the CLI can swallow exceptions and return exit code zero.
                "--ignore_page_error=False",
                "--raw_exceptions=True",
                "--multi_processing=False",
            ]),
            cancel,
            Duration::from_secs(300),
        )?;
        cancel.check()?;
        validate_editable_docx(&generated, cancel)?;
        progress(0.95);
        copy_output(&generated, output, cancel)?;
        progress(1.0);
        Ok(())
    }
}

fn validate_editable_docx(path: &Path, cancel: &CancelToken) -> Result<()> {
    let file = File::open(path).map_err(|e| io_error(path, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| invalid(e.to_string()))?;
    let entry = zip
        .by_name("word/document.xml")
        .map_err(|e| invalid(e.to_string()))?;
    const MAX_XML: u64 = 64 * 1024 * 1024;
    let mut xml = String::new();
    entry
        .take(MAX_XML + 1)
        .read_to_string(&mut xml)
        .map_err(|e| io_error(path, e))?;
    if xml.len() as u64 > MAX_XML {
        return Err(invalid("Converted document XML exceeds 64 MiB"));
    }
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut in_text = false;
    let mut editable = false;
    let mut depth = 0usize;
    let mut root_seen = false;
    loop {
        cancel.check()?;
        match reader.read_event().map_err(|e| invalid(e.to_string()))? {
            quick_xml::events::Event::Start(tag) => {
                if depth == 0 {
                    if root_seen || tag.name().as_ref() != "w:document" {
                        return Err(invalid("Invalid Word document root"));
                    }
                    root_seen = true;
                }
                depth += 1;
                if tag.name().as_ref() == "w:t" {
                    in_text = true;
                }
            }
            quick_xml::events::Event::End(tag) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("Invalid Word XML nesting"))?;
                if tag.name().as_ref() == "w:t" {
                    in_text = false;
                }
            }
            quick_xml::events::Event::Text(text) if in_text => {
                editable |= !text.as_ref().trim().is_empty();
            }
            quick_xml::events::Event::GeneralRef(text) if in_text => {
                let reference = format!("&{};", text.as_ref());
                editable |= !quick_xml::escape::unescape(&reference)
                    .map_err(|e| invalid(e.to_string()))?
                    .trim()
                    .is_empty();
            }
            quick_xml::events::Event::DocType(_) => {
                return Err(invalid("Unexpected document type in Word XML"))
            }
            quick_xml::events::Event::Eof => {
                if !root_seen || depth != 0 {
                    return Err(invalid("Incomplete Word XML"));
                }
                break;
            }
            _ => {}
        }
    }
    if !editable {
        return Err(invalid("Không tìm thấy chữ có thể chỉnh sửa. PDF scan cần OCR trước khi chuyển sang Word. / No editable text found; scanned PDFs need OCR before Word conversion."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn image_only_or_broken_outputs_are_not_reported_as_editable_word() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.docx");
        for (xml, valid) in [
            (
                r#"<w:document><w:body><w:p><w:r><w:t>Tiếng Việt</w:t></w:r></w:p></w:body></w:document>"#,
                true,
            ),
            (
                r#"<w:document><w:body><w:p><w:r><w:drawing/></w:r></w:p></w:body></w:document>"#,
                false,
            ),
            (
                r#"<w:document><w:body><w:p><w:r><w:t> </w:t></w:r></w:p></w:body></w:document>"#,
                false,
            ),
            (r#"<w:document><w:p><w:t>text</w:t></w:body>"#, false),
        ] {
            let mut zip = zip::ZipWriter::new(File::create(&output).unwrap());
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
            zip.finish().unwrap();
            assert_eq!(
                validate_editable_docx(&output, &CancelToken::new()).is_ok(),
                valid,
                "{xml}"
            );
        }
    }
}
