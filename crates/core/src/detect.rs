use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{Result, VertexError};
use crate::format::Format;

/// Detect file format using magic bytes first (via `infer`),
/// followed by content inspection (e.g. SVG / text / JSON),
/// and falling back to `mime_guess` and file extension.
pub fn detect_format<P: AsRef<Path>>(path: P) -> Result<Format> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(VertexError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "File does not exist"),
        });
    }

    // Try reading first 8KB for magic byte analysis
    let mut header = [0u8; 8192];
    let bytes_read = match File::open(path) {
        Ok(mut f) => f.read(&mut header).unwrap_or(0),
        Err(e) => {
            return Err(VertexError::Io {
                path: path.to_path_buf(),
                source: e,
            })
        }
    };

    let slice = &header[..bytes_read];

    // 1. Magic bytes via `infer`
    if let Some(kind) = infer::get(slice) {
        if let Some(format) = Format::from_mime(kind.mime_type()) {
            return Ok(format);
        }
        if let Some(format) = Format::from_extension(kind.extension()) {
            return Ok(format);
        }
    }

    // 2. Specific text checks (SVG, JSON, XML, MD, HTML)
    if let Ok(text) = std::str::from_utf8(slice) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
        {
            return Ok(Format::Svg);
        }
        if trimmed.starts_with("<!DOCTYPE html") || trimmed.starts_with("<html") {
            return Ok(Format::Html);
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                return Ok(Format::Json);
            }
        }
    }

    // 3. Mime guess from path
    if let Some(mime) = mime_guess::from_path(path).first() {
        if let Some(format) = Format::from_mime(mime.as_ref()) {
            return Ok(format);
        }
    }

    // 4. Extension fallback
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if let Some(format) = Format::from_extension(ext) {
            return Ok(format);
        }
    }

    Err(VertexError::UnrecognizedFormat(path.to_path_buf()))
}
