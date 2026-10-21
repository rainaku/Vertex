use super::external::{io_error, workspace};
use crate::converter::DetectedInput;
use crate::{
    detect_format, Availability, CancelToken, Converter, Format, Options, Result, VertexError,
};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

pub struct ArchiveConverter;
const SOURCES: &[Format] = &[Format::Zip, Format::TarGz, Format::SevenZ, Format::Gz];
const TARGETS: &[Format] = &[Format::Zip, Format::TarGz, Format::SevenZ];
const MAX_EXPANDED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const IO_BUFFER_SIZE: usize = 64 * 1024;
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
        let mut entries = Entries::new(target != Format::SevenZ, cancel);
        let file = BufReader::with_capacity(
            IO_BUFFER_SIZE,
            File::open(input.path).map_err(|e| io_error(input.path, e))?,
        );
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
                    let name = entry.name().map_err(invalid)?.into_owned();
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
        let output_file = BufWriter::with_capacity(
            IO_BUFFER_SIZE,
            File::create(&staged).map_err(|e| io_error(output, e))?,
        );
        match target {
            Format::Zip => {
                let mut writer = zip::ZipWriter::new(output_file);
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                for (name, data) in &entries.entries {
                    cancel.check()?;
                    match data {
                        EntryData::Directory(_) => {
                            writer
                                .add_directory(format!("{name}/"), options)
                                .map_err(invalid)?;
                        }
                        EntryData::Memory(bytes) => {
                            writer.start_file(name, options).map_err(invalid)?;
                            for chunk in bytes.chunks(IO_BUFFER_SIZE) {
                                cancel.check()?;
                                writer.write_all(chunk).map_err(invalid)?;
                            }
                        }
                        EntryData::Disk(path) => {
                            writer.start_file(name, options).map_err(invalid)?;
                            let mut file = File::open(path).map_err(invalid)?;
                            copy_checked(
                                &mut file,
                                &mut writer,
                                cancel,
                                &mut 0,
                                &mut entries.buffer,
                            )?;
                        }
                    }
                }
                writer.finish().map_err(invalid)?.flush().map_err(invalid)?;
            }
            Format::TarGz => {
                let mut writer =
                    tar::Builder::new(GzEncoder::new(output_file, Compression::default()));
                for (name, data) in &entries.entries {
                    cancel.check()?;
                    match data {
                        EntryData::Directory(Some(path)) => {
                            writer.append_dir(name, path).map_err(invalid)?;
                        }
                        EntryData::Directory(None) => {
                            let mut header = tar::Header::new_gnu();
                            header.set_entry_type(tar::EntryType::Directory);
                            header.set_size(0);
                            header.set_mode(0o755);
                            header.set_cksum();
                            writer
                                .append_data(&mut header, format!("{name}/"), std::io::empty())
                                .map_err(invalid)?;
                        }
                        EntryData::Memory(bytes) => {
                            let mut header = tar::Header::new_gnu();
                            header.set_entry_type(tar::EntryType::Regular);
                            header.set_size(bytes.len() as u64);
                            header.set_mode(0o644);
                            header.set_cksum();
                            writer
                                .append_data(
                                    &mut header,
                                    name,
                                    CancelReader(Cursor::new(bytes.as_slice()), cancel),
                                )
                                .map_err(|e| cancel.check().err().unwrap_or_else(|| invalid(e)))?;
                        }
                        EntryData::Disk(path) => {
                            let mut file = BufReader::with_capacity(
                                IO_BUFFER_SIZE,
                                File::open(path).map_err(invalid)?,
                            );
                            let mut header = tar::Header::new_gnu();
                            header.set_metadata(&file.get_ref().metadata().map_err(invalid)?);
                            header.set_cksum();
                            writer
                                .append_data(&mut header, name, CancelReader(&mut file, cancel))
                                .map_err(|e| cancel.check().err().unwrap_or_else(|| invalid(e)))?;
                        }
                    }
                }
                writer
                    .into_inner()
                    .map_err(invalid)?
                    .finish()
                    .map_err(invalid)?
                    .flush()
                    .map_err(invalid)?;
            }
            Format::SevenZ => {
                let mut writer = sevenz_rust2::ArchiveWriter::new(output_file).map_err(invalid)?;
                for (name, data) in &entries.entries {
                    cancel.check()?;
                    let (path, directory) = match data {
                        EntryData::Directory(Some(path)) => (path, true),
                        EntryData::Disk(path) => (path, false),
                        _ => unreachable!(),
                    };
                    let entry = sevenz_rust2::ArchiveEntry::from_path(path, name.clone());
                    let file = if directory {
                        None
                    } else {
                        Some(BufReader::with_capacity(
                            IO_BUFFER_SIZE,
                            File::open(path).map_err(invalid)?,
                        ))
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PathKind {
    File,
    ExplicitDir,
    ImplicitDir,
}

enum EntryData {
    Directory(Option<PathBuf>),
    Memory(Vec<u8>),
    Disk(PathBuf),
}

struct Entries<'a> {
    temp: Option<tempfile::TempDir>,
    use_memory: bool,
    entries: Vec<(String, EntryData)>,
    names: HashMap<String, PathKind>,
    bytes: u64,
    buffer: Box<[u8]>,
    cancel: &'a CancelToken,
}
impl<'a> Entries<'a> {
    fn new(use_memory: bool, cancel: &'a CancelToken) -> Self {
        Self {
            temp: None,
            use_memory,
            entries: Vec::new(),
            names: HashMap::new(),
            bytes: 0,
            buffer: vec![0u8; IO_BUFFER_SIZE].into_boxed_slice(),
            cancel,
        }
    }
    fn add(&mut self, raw: &str, directory: bool, reader: &mut dyn Read) -> Result<()> {
        self.cancel.check()?;
        if self.entries.len() >= MAX_ENTRIES {
            return Err(invalid("Archive exceeds 100,000 entries"));
        }
        let Some(name) = safe_name(raw)? else {
            return Ok(());
        };
        let key = name.to_lowercase();
        for (idx, _) in key.match_indices('/') {
            let ancestor = &key[..idx];
            match self.names.get(ancestor).copied() {
                Some(PathKind::File) => {
                    return Err(invalid(format!("Duplicate archive path: {name}")));
                }
                Some(PathKind::ExplicitDir | PathKind::ImplicitDir) => {}
                None => {
                    self.names
                        .insert(ancestor.to_owned(), PathKind::ImplicitDir);
                }
            }
        }
        match self.names.get(&key).copied() {
            Some(PathKind::ExplicitDir) if directory => return Ok(()),
            Some(PathKind::ImplicitDir) if directory => {
                self.names.insert(key, PathKind::ExplicitDir);
            }
            Some(_) => return Err(invalid(format!("Duplicate archive path: {name}"))),
            None => {
                self.names.insert(
                    key,
                    if directory {
                        PathKind::ExplicitDir
                    } else {
                        PathKind::File
                    },
                );
            }
        }
        if directory {
            if self.use_memory && self.bytes <= MAX_MEMORY_BYTES {
                self.entries.push((name, EntryData::Directory(None)));
            } else {
                if self.temp.is_none() {
                    self.temp = Some(workspace()?);
                }
                let path = self.temp.as_ref().unwrap().path().join(&name);
                std::fs::create_dir_all(&path).map_err(invalid)?;
                self.entries.push((name, EntryData::Directory(Some(path))));
            }
            return Ok(());
        }
        if self.use_memory && self.bytes <= MAX_MEMORY_BYTES {
            let mut data = Vec::new();
            loop {
                self.cancel.check()?;
                let n = reader.read(&mut self.buffer).map_err(invalid)?;
                if n == 0 {
                    self.entries.push((name, EntryData::Memory(data)));
                    return Ok(());
                }
                self.bytes += n as u64;
                if self.bytes > MAX_EXPANDED_BYTES {
                    return Err(invalid("Archive expands beyond the 8 GiB limit"));
                }
                if self.bytes <= MAX_MEMORY_BYTES {
                    data.extend_from_slice(&self.buffer[..n]);
                } else {
                    if self.temp.is_none() {
                        self.temp = Some(workspace()?);
                    }
                    let path = self.temp.as_ref().unwrap().path().join(&name);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(invalid)?;
                    }
                    let mut file = BufWriter::with_capacity(
                        IO_BUFFER_SIZE,
                        OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .map_err(invalid)?,
                    );
                    if !data.is_empty() {
                        file.write_all(&data).map_err(invalid)?;
                    }
                    drop(data);
                    file.write_all(&self.buffer[..n]).map_err(invalid)?;
                    copy_checked(
                        reader,
                        &mut file,
                        self.cancel,
                        &mut self.bytes,
                        &mut self.buffer,
                    )?;
                    file.flush().map_err(invalid)?;
                    self.entries.push((name, EntryData::Disk(path)));
                    return Ok(());
                }
            }
        }
        if self.temp.is_none() {
            self.temp = Some(workspace()?);
        }
        let path = self.temp.as_ref().unwrap().path().join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(invalid)?;
        }
        let mut file = BufWriter::with_capacity(
            IO_BUFFER_SIZE,
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(invalid)?,
        );
        copy_checked(
            reader,
            &mut file,
            self.cancel,
            &mut self.bytes,
            &mut self.buffer,
        )?;
        file.flush().map_err(invalid)?;
        self.entries.push((name, EntryData::Disk(path)));
        Ok(())
    }
}

fn is_windows_device(base: &str) -> bool {
    if base.eq_ignore_ascii_case("CON")
        || base.eq_ignore_ascii_case("PRN")
        || base.eq_ignore_ascii_case("AUX")
        || base.eq_ignore_ascii_case("NUL")
        || base.eq_ignore_ascii_case("CONIN$")
        || base.eq_ignore_ascii_case("CONOUT$")
    {
        return true;
    }
    if let Some((prefix, rest)) = base.split_at_checked(3) {
        if (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
            && matches!(
                rest,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        {
            return true;
        }
    }
    false
}

fn safe_name(raw: &str) -> Result<Option<String>> {
    let name: Cow<'_, str> = if raw.contains('\\') {
        Cow::Owned(raw.replace('\\', "/"))
    } else {
        Cow::Borrowed(raw)
    };
    let path = Path::new(name.as_ref());
    if name.starts_with('/')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(invalid(format!("Unsafe archive path: {raw}")));
    }
    let trimmed = name.trim_end_matches('/');
    let mut saw_dot = false;
    let mut kept = 0usize;
    for part in trimmed.split('/') {
        if part.is_empty() {
            return Err(invalid(format!("Unsafe archive path: {raw}")));
        }
        if part == "." {
            saw_dot = true;
            continue;
        }
        kept += 1;
        let base = part.split('.').next().unwrap_or("");
        if part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || is_windows_device(base)
        {
            return Err(invalid(format!("Unsafe archive path: {raw}")));
        }
    }
    if kept == 0 {
        return Ok(None);
    }
    if !saw_dot {
        let trimmed_len = trimmed.len();
        return match name {
            Cow::Owned(mut s) => {
                s.truncate(trimmed_len);
                Ok(Some(s))
            }
            Cow::Borrowed(s) => Ok(Some(s[..trimmed_len].to_owned())),
        };
    }
    let mut out = String::with_capacity(trimmed.len());
    for part in trimmed.split('/').filter(|&part| part != ".") {
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    Ok(Some(out))
}

fn copy_checked(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    cancel: &CancelToken,
    bytes: &mut u64,
    buffer: &mut [u8],
) -> Result<()> {
    loop {
        cancel.check()?;
        let n = reader.read(buffer).map_err(invalid)?;
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
