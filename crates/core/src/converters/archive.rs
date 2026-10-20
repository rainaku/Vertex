use super::external::{io_error, workspace};
use crate::converter::DetectedInput;
use crate::{
    detect_format, Availability, CancelToken, Converter, Format, Options, Result, VertexError,
};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub struct ArchiveConverter;
const SOURCES: &[Format] = &[Format::Zip, Format::TarGz, Format::SevenZ, Format::Gz];
const TARGETS: &[Format] = &[Format::Zip, Format::TarGz, Format::SevenZ];
const MAX_EXPANDED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
fn invalid(e: impl std::fmt::Display) -> VertexError {
    VertexError::Encoding(e.to_string())
}

impl Converter for ArchiveConverter {
    fn name(&self) -> &'static str {
        "ArchiveConverter"
    }
    fn sources(&self) -> &[Format] {
        SOURCES
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        TARGETS.to_vec()
    }
    fn available(&self) -> Availability {
        Availability::Ok
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
        _: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input.path, output)?;
        let target = Format::from_path_extension(output)
            .ok_or_else(|| VertexError::UnrecognizedFormat(output.into()))?;
        if !SOURCES.contains(&input.format) || !TARGETS.contains(&target) {
            return Err(invalid("Unsupported archive conversion"));
        }
        let temp = workspace()?;
        let mut entries = Entries {
            root: temp.path(),
            paths: Vec::new(),
            names: HashSet::new(),
            bytes: 0,
            cancel,
        };
        let file = File::open(input.path).map_err(|e| io_error(input.path, e))?;
        match input.format {
            Format::Zip => {
                let mut archive = zip::ZipArchive::new(file).map_err(invalid)?;
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).map_err(invalid)?;
                    if entry
                        .unix_mode()
                        .is_some_and(|mode| mode & 0o170000 == 0o120000)
                    {
                        return Err(invalid("Archive links are unsupported"));
                    }
                    let name = entry.name().to_owned();
                    entries.add(&name, entry.is_dir(), &mut entry)?;
                }
            }
            Format::TarGz => {
                let mut archive = tar::Archive::new(MultiGzDecoder::new(file));
                for entry in archive.entries().map_err(invalid)? {
                    let mut entry = entry.map_err(invalid)?;
                    let kind = entry.header().entry_type();
                    if !kind.is_file() && !kind.is_dir() {
                        return Err(invalid("Archive links and special files are unsupported"));
                    }
                    let name = entry
                        .path()
                        .map_err(invalid)?
                        .to_str()
                        .ok_or_else(|| invalid("Archive names must be UTF-8"))?
                        .to_owned();
                    entries.add(&name, kind.is_dir(), &mut entry)?;
                }
            }
            Format::SevenZ => {
                let mut archive =
                    sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
                        .map_err(invalid)?;
                let mut failure = None;
                let result = archive.for_each_entries(|entry, reader| {
                    let mode = entry.windows_attributes >> 16;
                    let result =
                        if mode & 0o170000 == 0o120000 || entry.windows_attributes & 0x400 != 0 {
                            Err(invalid("Archive links are unsupported"))
                        } else {
                            entries.add(&entry.name, entry.is_directory, reader)
                        };
                    match result {
                        Ok(()) => Ok(true),
                        Err(error) => {
                            failure = Some(error);
                            Ok(false)
                        }
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
                result.map_err(invalid)?;
            }
            Format::Gz => {
                // Ignore the untrusted gzip header filename; use the input basename.
                let name = input
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("data");
                entries.add(name, false, &mut MultiGzDecoder::new(file))?;
            }
            _ => unreachable!(),
        }
        progress(0.5);
        let staged = crate::path_utils::stage_output(output)?;
        let output_file = File::create(&staged).map_err(|e| io_error(output, e))?;
        match target {
            Format::Zip => {
                let mut writer = zip::ZipWriter::new(output_file);
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                for (name, path, directory) in &entries.paths {
                    cancel.check()?;
                    if *directory {
                        writer
                            .add_directory(format!("{name}/"), options)
                            .map_err(invalid)?;
                    } else {
                        writer.start_file(name, options).map_err(invalid)?;
                        copy_checked(
                            &mut File::open(path).map_err(invalid)?,
                            &mut writer,
                            cancel,
                            &mut 0,
                        )?;
                    }
                }
                writer.finish().map_err(invalid)?;
            }
            Format::TarGz => {
                let mut writer =
                    tar::Builder::new(GzEncoder::new(output_file, Compression::default()));
                for (name, path, directory) in &entries.paths {
                    cancel.check()?;
                    if *directory {
                        writer.append_dir(name, path).map_err(invalid)?;
                    } else {
                        let mut file = File::open(path).map_err(invalid)?;
                        let mut header = tar::Header::new_gnu();
                        header.set_metadata(&file.metadata().map_err(invalid)?);
                        header.set_cksum();
                        writer
                            .append_data(&mut header, name, CancelReader(&mut file, cancel))
                            .map_err(|e| cancel.check().err().unwrap_or_else(|| invalid(e)))?;
                    }
                }
                writer
                    .into_inner()
                    .map_err(invalid)?
                    .finish()
                    .map_err(invalid)?;
            }
            Format::SevenZ => {
                let mut writer = sevenz_rust2::ArchiveWriter::new(output_file).map_err(invalid)?;
                for (name, path, directory) in &entries.paths {
                    cancel.check()?;
                    let entry = sevenz_rust2::ArchiveEntry::from_path(path, name.clone());
                    let file = if *directory {
                        None
                    } else {
                        Some(File::open(path).map_err(invalid)?)
                    };
                    writer
                        .push_archive_entry(entry, file.map(|f| CancelReader(f, cancel)))
                        .map_err(|e| cancel.check().err().unwrap_or_else(|| invalid(e)))?;
                }
                writer.finish().map_err(invalid)?;
            }
            _ => unreachable!(),
        }
        cancel.check()?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}

struct Entries<'a> {
    root: &'a Path,
    paths: Vec<(String, PathBuf, bool)>,
    names: HashSet<String>,
    bytes: u64,
    cancel: &'a CancelToken,
}
impl Entries<'_> {
    fn add(&mut self, raw: &str, directory: bool, reader: &mut dyn Read) -> Result<()> {
        self.cancel.check()?;
        if self.paths.len() >= MAX_ENTRIES {
            return Err(invalid("Archive exceeds 100,000 entries"));
        }
        let name = safe_name(raw)?;
        if !self.names.insert(name.to_lowercase()) {
            return Err(invalid(format!("Duplicate archive path: {name}")));
        }
        let path = self.root.join(&name);
        if directory {
            std::fs::create_dir_all(&path).map_err(invalid)?;
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(invalid)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(invalid)?;
            copy_checked(reader, &mut file, self.cancel, &mut self.bytes)?;
        }
        self.paths.push((name, path, directory));
        Ok(())
    }
}

fn safe_name(raw: &str) -> Result<String> {
    let name = raw.replace('\\', "/");
    let path = Path::new(&name);
    if name.starts_with('/')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid(format!("Unsafe archive path: {raw}")));
    }
    let parts: Vec<_> = name.trim_end_matches('/').split('/').collect();
    for part in &parts {
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let device = matches!(
            base.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|p| {
            base.strip_prefix(p).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if part.is_empty()
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || device
        {
            return Err(invalid(format!("Unsafe archive path: {raw}")));
        }
    }
    Ok(parts.join("/"))
}

fn copy_checked(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    cancel: &CancelToken,
    bytes: &mut u64,
) -> Result<()> {
    let mut buffer = [0; 64 * 1024];
    loop {
        cancel.check()?;
        let n = reader.read(&mut buffer).map_err(invalid)?;
        if n == 0 {
            return Ok(());
        }
        *bytes += n as u64;
        if *bytes > MAX_EXPANDED_BYTES {
            return Err(invalid("Archive expands beyond the 8 GiB limit"));
        }
        writer.write_all(&buffer[..n]).map_err(invalid)?;
    }
}
struct CancelReader<'a, R>(R, &'a CancelToken);
impl<R: Read> Read for CancelReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_cancelled() {
            return Err(std::io::Error::other("Operation cancelled"));
        }
        self.0.read(buffer)
    }
}
