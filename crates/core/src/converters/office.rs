use super::external::{copy_output, find_tool, io_error, run_tool, workspace};
use crate::converter::DetectedInput;
use crate::{
    detect_format, Availability, CancelToken, Converter, Format, Options, Result, VertexError,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub struct OfficeConverter;
const DOCUMENTS: &[Format] = &[
    Format::Doc,
    Format::Docx,
    Format::Docm,
    Format::Dot,
    Format::Dotx,
    Format::Dotm,
    Format::Odt,
    Format::Ott,
    Format::Fodt,
    Format::Sxw,
    Format::Wps,
    Format::Wpd,
    Format::Wri,
    Format::Rtf,
    Format::Txt,
    Format::Html,
    Format::Md,
];
const SHEETS: &[Format] = &[
    Format::Xlsx,
    Format::Xls,
    Format::Ods,
    Format::Csv,
    Format::Tsv,
];
const SLIDES: &[Format] = &[Format::Pptx, Format::Ppt, Format::Odp];
const SOURCES: &[Format] = &[
    Format::Doc,
    Format::Docx,
    Format::Docm,
    Format::Dot,
    Format::Dotx,
    Format::Dotm,
    Format::Odt,
    Format::Ott,
    Format::Fodt,
    Format::Sxw,
    Format::Wps,
    Format::Wpd,
    Format::Wri,
    Format::Rtf,
    Format::Txt,
    Format::Html,
    Format::Md,
    Format::Xlsx,
    Format::Xls,
    Format::Ods,
    Format::Csv,
    Format::Tsv,
    Format::Pptx,
    Format::Ppt,
    Format::Odp,
];

fn binary() -> Option<PathBuf> {
    let locations = &[
        "LibreOffice/program",
        "/usr/bin",
        "/opt/libreoffice/program",
        "/Applications/LibreOffice.app/Contents/MacOS",
    ];
    // The Windows console launcher waits for soffice.bin to finish, which is
    // required before checking and publishing headless conversion output.
    #[cfg(windows)]
    if let Some(path) = find_tool("soffice.com", locations) {
        return Some(path);
    }
    find_tool("soffice", locations)
}

fn export_filter(target: Format, source: Format) -> &'static str {
    match target {
        Format::Pdf if SHEETS.contains(&source) => "pdf:calc_pdf_Export",
        Format::Pdf if SLIDES.contains(&source) => "pdf:impress_pdf_Export",
        Format::Pdf => "pdf:writer_pdf_Export",
        Format::Doc => "doc:MS Word 97",
        Format::Docx => "docx:Office Open XML Text",
        Format::Dotx => "dotx:Office Open XML Text Template",
        Format::Odt => "odt:writer8",
        Format::Ott => "ott:writer8_template",
        Format::Fodt => "fodt:OpenDocument Text Flat XML",
        Format::Rtf => "rtf:Rich Text Format",
        Format::Txt => "txt:Text (encoded):UTF8",
        Format::Html => "html:HTML (StarWriter):EmbedImages",
        Format::Xlsx => "xlsx:Calc Office Open XML",
        Format::Xls => "xls:MS Excel 97",
        Format::Ods => "ods:calc8",
        Format::Csv => {
            "csv:Text - txt - csv (StarCalc):44,34,76,1,,0,false,true,false,false,false,0"
        }
        Format::Tsv => {
            "csv:Text - txt - csv (StarCalc):9,34,76,1,,0,false,true,false,false,false,0"
        }
        Format::Pptx => "pptx:Impress Office Open XML",
        Format::Ppt => "ppt:MS PowerPoint 97",
        Format::Odp => "odp:impress8",
        _ => unreachable!(),
    }
}

fn document_import_filter(format: Format) -> Option<&'static str> {
    Some(match format {
        Format::Docm => "MS Word 2007 XML VBA",
        Format::Dot => "MS Word 97 Vorlage",
        Format::Dotx => "Office Open XML Text Template",
        Format::Dotm => "MS Word 2007 XML Template",
        Format::Ott => "writer8_template",
        Format::Fodt => "OpenDocument Text Flat XML",
        Format::Sxw => "StarOffice XML (Writer)",
        Format::Wps => "MS_Works",
        Format::Wpd => "WordPerfect",
        Format::Wri => "MS_Write",
        _ => return None,
    })
}

fn table_import_filter(path: &Path, format: Format) -> Result<String> {
    let delimiter = if format == Format::Tsv { b'\t' } else { b',' };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .from_path(path)
        .map_err(|e| VertexError::Encoding(e.to_string()))?;
    let columns = reader
        .records()
        .next()
        .transpose()
        .map_err(|e| VertexError::Encoding(e.to_string()))?
        .map_or(0, |row| row.len());
    if columns > 16384 {
        return Err(VertexError::Encoding(
            "Workbook input exceeds 16,384 columns".into(),
        ));
    }
    let formats = (1..=columns)
        .map(|column| format!("{column}/2"))
        .collect::<Vec<_>>()
        .join("/");
    // CSV has no type system: retain leading zeroes, large integers and formula-
    // looking text, rather than evaluating formulas during import.
    Ok(format!("Text - txt - csv (StarCalc):{delimiter},34,76,1,{formats},0,true,false,false,false,false,0,false,false,false"))
}

impl Converter for OfficeConverter {
    fn name(&self) -> &'static str {
        "OfficeConverter"
    }
    fn sources(&self) -> &[Format] {
        SOURCES
    }
    fn targets(&self, from: Format) -> Vec<Format> {
        let mut targets = if DOCUMENTS.contains(&from) {
            vec![
                Format::Doc,
                Format::Docx,
                Format::Dotx,
                Format::Odt,
                Format::Ott,
                Format::Fodt,
                Format::Rtf,
                Format::Txt,
                Format::Html,
            ]
        } else if SHEETS.contains(&from) {
            SHEETS.to_vec()
        } else if SLIDES.contains(&from) {
            SLIDES.to_vec()
        } else {
            return Vec::new();
        };
        targets.retain(|f| *f != from);
        targets.push(Format::Pdf);
        targets
    }
    fn available(&self) -> Availability {
        if binary().is_some() {
            Availability::Ok
        } else {
            Availability::Missing("Tài liệu, bảng tính và trình chiếu cần LibreOffice. Cài LibreOffice và khởi động lại Vertex. / Office conversion requires LibreOffice.".into())
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
        input: DetectedInput<'_>,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input.path, output)?;
        let target = Format::from_path_extension(output)
            .ok_or_else(|| VertexError::UnrecognizedFormat(output.into()))?;
        if !self.targets(input.format).contains(&target) {
            return Err(VertexError::NoConversionRoute {
                from: input.format.to_string(),
                to: target.to_string(),
            });
        }
        let binary = binary().ok_or_else(|| VertexError::EngineMissing("LibreOffice".into()))?;
        let temp = workspace()?;
        let source_dir = temp.path().join("input");
        let output_dir = temp.path().join("output");
        let profile = temp.path().join("profile");
        for directory in [&source_dir, &output_dir, &profile.join("user")] {
            std::fs::create_dir_all(directory).map_err(|e| io_error(directory, e))?;
        }
        // Dedicated profiles avoid forwarding work to a running LibreOffice UI,
        // isolate concurrent jobs, and disable document macros and link updates.
        std::fs::write(profile.join("user/registrymodifications.xcu"), r#"<?xml version="1.0" encoding="UTF-8"?><oor:items xmlns:oor="http://openoffice.org/2001/registry"><item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop></item><item oor:path="/org.openoffice.Office.Writer/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item><item oor:path="/org.openoffice.Office.Calc/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item></oor:items>"#).map_err(|e| io_error(&profile, e))?;
        let source = if input.format == Format::Md {
            let source = source_dir.join("source.html");
            super::data::TextConverter.convert_detected(input, &source, opts, &|_| {}, cancel)?;
            source
        } else if input.format == Format::Txt {
            let source = source_dir.join("source.txt");
            let text = super::text::read_unicode(input.path, cancel)?;
            std::fs::write(&source, text).map_err(|e| io_error(&source, e))?;
            source
        } else {
            let source = source_dir.join(format!("source.{}", input.format.extension()));
            std::fs::copy(input.path, &source).map_err(|e| io_error(input.path, e))?;
            source
        };
        let profile_url = url::Url::from_directory_path(&profile)
            .map_err(|_| VertexError::InvalidOptions("Invalid LibreOffice profile path".into()))?;
        let mut command = Command::new(binary);
        command
            .arg(format!("-env:UserInstallation={profile_url}"))
            .args(["--headless", "--nologo", "--nodefault", "--norestore"]);
        if matches!(input.format, Format::Csv | Format::Tsv) {
            command.arg(format!(
                "--infilter={}",
                table_import_filter(&source, input.format)?
            ));
        } else if input.format == Format::Txt {
            command.arg("--infilter=Text (encoded):UTF8,LF");
        } else if let Some(filter) = document_import_filter(input.format) {
            command.arg(format!("--infilter={filter}"));
        }
        let log = run_tool(
            command
                .arg("--convert-to")
                .arg(export_filter(target, input.format))
                .arg("--outdir")
                .arg(&output_dir)
                .arg(&source),
            cancel,
            Duration::from_secs(300),
        )?;
        let generated = output_dir.join(format!(
            "source.{}",
            if target == Format::Tsv {
                "csv"
            } else {
                target.extension()
            }
        ));
        // LibreOffice can exit with code zero when an import/export filter fails.
        if !generated.is_file() {
            return Err(VertexError::Encoding(format!(
                "LibreOffice produced no {} output: {log}",
                target
            )));
        }
        copy_output(&generated, output, cancel)?;
        progress(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_filters_use_utf8_explicit_delimiters_and_safe_text_import() {
        let temp = tempfile::tempdir().unwrap();
        for (format, delimiter, text) in [
            (Format::Csv, "44", "id,value\n001,=1+1\n"),
            (Format::Tsv, "9", "id\tvalue\n001\t=1+1\n"),
        ] {
            let path = temp.path().join(format!("table.{}", format.extension()));
            std::fs::write(&path, text).unwrap();
            let import = table_import_filter(&path, format).unwrap();
            let tokens: Vec<_> = import.split(':').nth(1).unwrap().split(',').collect();
            assert_eq!(&tokens[..5], &[delimiter, "34", "76", "1", "1/2/2/2"]);
            assert_eq!(tokens[12], "false"); // formulas remain text
            let export = export_filter(format, Format::Xlsx);
            assert_eq!(
                export.rsplit(':').next().unwrap().split(',').nth(11),
                Some("0")
            ); // first sheet
        }
    }
    #[test]
    fn office_filters_select_the_correct_document_family_and_embedded_html_images() {
        assert_eq!(
            export_filter(Format::Pdf, Format::Docx),
            "pdf:writer_pdf_Export"
        );
        assert_eq!(
            export_filter(Format::Pdf, Format::Xlsx),
            "pdf:calc_pdf_Export"
        );
        assert_eq!(
            export_filter(Format::Pdf, Format::Pptx),
            "pdf:impress_pdf_Export"
        );
        assert!(export_filter(Format::Html, Format::Docx).ends_with(":EmbedImages"));
    }
}
