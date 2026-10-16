use crate::error::{Result, VertexError};
use crate::format::Format;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Content wins over filenames; extensions disambiguate shared containers or
/// formats without a reliable signature. JSON inspection never builds a DOM.
pub fn detect_format<P: AsRef<Path>>(path: P) -> Result<Format> {
    let path = path.as_ref();
    let io = |source| VertexError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = File::open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Expected a regular file",
        )));
    }
    let extension = Format::from_path_extension(path);
    let mut header = [0; 8192];
    let count = read_prefix(&mut file, &mut header).map_err(io)?;
    let mut bytes = &header[..count];
    let mut payload = [0; 8192];
    if bytes.starts_with(b"ID3") && bytes.len() >= 10 && bytes[6..10].iter().all(|b| b & 0x80 == 0)
    {
        let size = bytes[6..10]
            .iter()
            .fold(0u64, |n, b| (n << 7) | u64::from(*b));
        let footer = if bytes[3] == 4 && bytes[5] & 0x10 != 0 {
            10
        } else {
            0
        };
        file.seek(SeekFrom::Start(10 + size + footer)).map_err(io)?;
        let n = read_prefix(&mut file, &mut payload).map_err(io)?;
        if n > 0 {
            bytes = &payload[..n];
        }
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        file.rewind().map_err(io)?;
        if let Ok(mut archive) = zip::ZipArchive::new(&mut file) {
            for (name, format) in [
                ("word/document.xml", Format::Docx),
                ("xl/workbook.xml", Format::Xlsx),
                ("ppt/presentation.xml", Format::Pptx),
            ] {
                if archive.index_for_name(name).is_some() {
                    return Ok(format);
                }
            }
            if let Ok(entry) = archive.by_name("mimetype") {
                let mut mime = String::new();
                if entry.take(256).read_to_string(&mut mime).is_ok() {
                    if let Some(format) = Format::from_mime(mime.trim()) {
                        return Ok(format);
                    }
                }
            };
        }
        return Ok(Format::Zip);
    }
    if bytes.starts_with(b"\x1aE\xdf\xa3") {
        for (i, tag) in bytes.windows(3).enumerate() {
            if tag[..2] == [0x42, 0x82] && tag[2] & 0x80 != 0 {
                let len = usize::from(tag[2] & 0x7f);
                match bytes.get(i + 3..i + 3 + len) {
                    Some(b"matroska") => return Ok(Format::Mkv),
                    Some(b"webm") => return Ok(Format::Webm),
                    _ => {}
                }
            }
        }
        return Ok(if extension == Some(Format::Webm) {
            Format::Webm
        } else {
            Format::Mkv
        });
    }
    if bytes.starts_with(b"OggS") && bytes.windows(8).any(|w| w == b"OpusHead") {
        return Ok(Format::Opus);
    }
    if bytes.get(4..8) == Some(b"ftyp") {
        let brand = bytes.get(8..12).unwrap_or_default();
        if brand.starts_with(b"3g") {
            return Ok(Format::ThreeGp);
        }
        if brand == b"M4A " || brand == b"M4B " {
            return Ok(Format::M4a);
        }
        if matches!(extension, Some(Format::M4a | Format::ThreeGp))
            && matches!(brand, b"isom" | b"mp41" | b"mp42")
        {
            return Ok(extension.unwrap());
        }
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(e) if e.error_len().is_none() => std::str::from_utf8(&bytes[..e.valid_up_to()]).ok(),
        _ => None,
    };
    if let Some(text) = text {
        let text = text.trim_start_matches('\u{feff}').trim_start();
        if text.starts_with('<') {
            let mut reader = quick_xml::Reader::from_str(text);
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e)) => {
                        if e.local_name().as_ref() == "svg" {
                            return Ok(Format::Svg);
                        }
                        if e.local_name().as_ref().eq_ignore_ascii_case("html") {
                            return Ok(Format::Html);
                        }
                        break;
                    }
                    Ok(quick_xml::events::Event::Eof) | Err(_) => break,
                    _ => {}
                }
            }
            if text
                .get(..14)
                .is_some_and(|s| s.eq_ignore_ascii_case("<!doctype html"))
            {
                return Ok(Format::Html);
            }
        }
        if text.starts_with('{') || text.starts_with('[') {
            file.rewind().map_err(io)?;
            if bytes.starts_with(b"\xef\xbb\xbf") {
                file.seek(SeekFrom::Start(3)).map_err(io)?;
            }
            let mut deserializer = serde_json::Deserializer::from_reader(BufReader::new(&mut file));
            if serde::de::IgnoredAny::deserialize(&mut deserializer).is_ok()
                && deserializer.end().is_ok()
            {
                return Ok(Format::Json);
            }
        }
    }
    if let Some(kind) = infer::get(bytes) {
        if let Some(format) =
            Format::from_mime(kind.mime_type()).or_else(|| Format::from_extension(kind.extension()))
        {
            return Ok(match (format, extension) {
                (Format::Wmv, Some(Format::Wma)) => Format::Wma,
                (Format::Ogg, Some(Format::Opus)) => Format::Opus,
                _ => format,
            });
        }
    }
    extension.ok_or_else(|| VertexError::UnrecognizedFormat(path.to_path_buf()))
}

fn read_prefix(reader: &mut impl Read, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
