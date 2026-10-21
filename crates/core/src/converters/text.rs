//! Decode Unicode explicitly; never replace invalid bytes or guess an ANSI code page.
use super::external::io_error;
use crate::{CancelToken, Result, VertexError};
use std::io::Read;
use std::path::Path;

const MAX_TEXT_SIZE: u64 = 64 * 1024 * 1024;

pub(super) fn read_unicode(path: &Path, cancel: &CancelToken) -> Result<String> {
    cancel.check()?;
    let file = std::fs::File::open(path).map_err(|e| io_error(path, e))?;
    let cap = file
        .metadata()
        .map(|m| m.len().min(MAX_TEXT_SIZE + 1) as usize)
        .unwrap_or(0);
    let mut bytes = Vec::with_capacity(cap);
    file.take(MAX_TEXT_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(path, e))?;
    cancel.check()?;
    if bytes.len() as u64 > MAX_TEXT_SIZE {
        return Err(VertexError::Encoding(
            "Text input is limited to 64 MiB per conversion".into(),
        ));
    }
    decode_unicode(bytes)
}

fn decode_unicode(bytes: Vec<u8>) -> Result<String> {
    let invalid = || {
        VertexError::Encoding(
        "Không đọc được mã hóa văn bản. Hãy lưu file bằng UTF-8 hoặc UTF-16 có BOM. / Invalid text encoding; save as UTF-8 or UTF-16 with a BOM.".into(),
    )
    };
    // UTF-32LE shares UTF-16LE's prefix; reject it before the UTF-16 branch.
    if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        return Err(invalid());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little_endian = bytes[0] == 0xff;
        let (pairs, remainder) = bytes[2..].as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(invalid());
        }
        let mut text = String::with_capacity(pairs.len());
        if little_endian {
            for r in char::decode_utf16(pairs.iter().copied().map(u16::from_le_bytes)) {
                let c = r.map_err(|_| invalid())?;
                if c == '\0' {
                    return Err(invalid());
                }
                text.push(c);
            }
        } else {
            for r in char::decode_utf16(pairs.iter().copied().map(u16::from_be_bytes)) {
                let c = r.map_err(|_| invalid())?;
                if c == '\0' {
                    return Err(invalid());
                }
                text.push(c);
            }
        }
        Ok(text)
    } else {
        // NULs usually indicate BOM-less UTF-16 or binary input. Don't silently emit
        // corrupted text that another converter might then discard.
        if bytes.contains(&0) {
            return Err(invalid());
        }
        let mut text = String::from_utf8(bytes).map_err(|_| invalid())?;
        if text.starts_with('\u{feff}') {
            text.drain(..3);
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_encodings_retain_vietnamese_whitespace_and_supplementary_characters() {
        let text = "\r\nTiếng Việt\t  có dấu 😀\r\n\r\n";
        assert_eq!(decode_unicode(text.as_bytes().to_vec()).unwrap(), text);
        let mut utf8 = vec![0xef, 0xbb, 0xbf];
        utf8.extend_from_slice(text.as_bytes());
        assert_eq!(decode_unicode(utf8).unwrap(), text);
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for word in text.encode_utf16() {
                bytes.extend_from_slice(&if little {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            assert_eq!(decode_unicode(bytes).unwrap(), text);
        }
    }

    #[test]
    fn invalid_encodings_are_not_silently_replaced() {
        for bytes in [
            &[0xff][..],
            &[0xff, 0xfe, 65],
            &[0xff, 0xfe, 0, 0xd8],
            &[0xff, 0xfe, 0, 0, 65, 0, 0, 0],
            b"A\0B\0",
        ] {
            assert!(decode_unicode(bytes.to_vec()).is_err());
        }
    }
}
