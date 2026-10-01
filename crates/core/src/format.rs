use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Image,
    Document,
    Spreadsheet,
    Presentation,
    Audio,
    Video,
    Archive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Format {
    // Images
    Png,
    Jpg,
    Webp,
    Avif,
    Heic,
    Tiff,
    Bmp,
    Gif,
    Ico,
    Svg,

    // Documents
    Pdf,
    Docx,
    Odt,
    Rtf,
    Txt,
    Md,
    Html,

    // Spreadsheets & Data
    Xlsx,
    Xls,
    Ods,
    Csv,
    Tsv,
    Json,
    Yaml,
    Toml,
    Xml,

    // Presentations
    Pptx,
    Ppt,
    Odp,

    // Audio
    Mp3,
    Wav,
    Flac,
    Aac,
    Ogg,
    M4a,
    Opus,
    Aiff,
    Wma,

    // Video
    Mp4,
    Mov,
    Mkv,
    Avi,
    Webm,
    Wmv,
    Flv,
    ThreeGp,
    Ts,

    // Archives
    Zip,
    TarGz,
    SevenZ,
}

impl Format {
    pub fn label(&self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpg => "JPG",
            Format::Webp => "WEBP",
            Format::Avif => "AVIF",
            Format::Heic => "HEIC",
            Format::Tiff => "TIFF",
            Format::Bmp => "BMP",
            Format::Gif => "GIF",
            Format::Ico => "ICO",
            Format::Svg => "SVG",
            Format::Pdf => "PDF",
            Format::Docx => "DOCX",
            Format::Odt => "ODT",
            Format::Rtf => "RTF",
            Format::Txt => "TXT",
            Format::Md => "MD",
            Format::Html => "HTML",
            Format::Xlsx => "XLSX",
            Format::Xls => "XLS",
            Format::Ods => "ODS",
            Format::Csv => "CSV",
            Format::Tsv => "TSV",
            Format::Json => "JSON",
            Format::Yaml => "YAML",
            Format::Toml => "TOML",
            Format::Xml => "XML",
            Format::Pptx => "PPTX",
            Format::Ppt => "PPT",
            Format::Odp => "ODP",
            Format::Mp3 => "MP3",
            Format::Wav => "WAV",
            Format::Flac => "FLAC",
            Format::Aac => "AAC",
            Format::Ogg => "OGG",
            Format::M4a => "M4A",
            Format::Opus => "OPUS",
            Format::Aiff => "AIFF",
            Format::Wma => "WMA",
            Format::Mp4 => "MP4",
            Format::Mov => "MOV",
            Format::Mkv => "MKV",
            Format::Avi => "AVI",
            Format::Webm => "WEBM",
            Format::Wmv => "WMV",
            Format::Flv => "FLV",
            Format::ThreeGp => "3GP",
            Format::Ts => "TS",
            Format::Zip => "ZIP",
            Format::TarGz => "TAR.GZ",
            Format::SevenZ => "7Z",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg => "jpg",
            Format::Webp => "webp",
            Format::Avif => "avif",
            Format::Heic => "heic",
            Format::Tiff => "tiff",
            Format::Bmp => "bmp",
            Format::Gif => "gif",
            Format::Ico => "ico",
            Format::Svg => "svg",
            Format::Pdf => "pdf",
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Rtf => "rtf",
            Format::Txt => "txt",
            Format::Md => "md",
            Format::Html => "html",
            Format::Xlsx => "xlsx",
            Format::Xls => "xls",
            Format::Ods => "ods",
            Format::Csv => "csv",
            Format::Tsv => "tsv",
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Xml => "xml",
            Format::Pptx => "pptx",
            Format::Ppt => "ppt",
            Format::Odp => "odp",
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
            Format::Flac => "flac",
            Format::Aac => "aac",
            Format::Ogg => "ogg",
            Format::M4a => "m4a",
            Format::Opus => "opus",
            Format::Aiff => "aiff",
            Format::Wma => "wma",
            Format::Mp4 => "mp4",
            Format::Mov => "mov",
            Format::Mkv => "mkv",
            Format::Avi => "avi",
            Format::Webm => "webm",
            Format::Wmv => "wmv",
            Format::Flv => "flv",
            Format::ThreeGp => "3gp",
            Format::Ts => "ts",
            Format::Zip => "zip",
            Format::TarGz => "tar.gz",
            Format::SevenZ => "7z",
        }
    }

    pub fn category(&self) -> Category {
        match self {
            Format::Png
            | Format::Jpg
            | Format::Webp
            | Format::Avif
            | Format::Heic
            | Format::Tiff
            | Format::Bmp
            | Format::Gif
            | Format::Ico
            | Format::Svg => Category::Image,

            Format::Pdf
            | Format::Docx
            | Format::Odt
            | Format::Rtf
            | Format::Txt
            | Format::Md
            | Format::Html => Category::Document,

            Format::Xlsx
            | Format::Xls
            | Format::Ods
            | Format::Csv
            | Format::Tsv
            | Format::Json
            | Format::Yaml
            | Format::Toml
            | Format::Xml => Category::Spreadsheet,

            Format::Pptx | Format::Ppt | Format::Odp => Category::Presentation,

            Format::Mp3
            | Format::Wav
            | Format::Flac
            | Format::Aac
            | Format::Ogg
            | Format::M4a
            | Format::Opus
            | Format::Aiff
            | Format::Wma => Category::Audio,

            Format::Mp4
            | Format::Mov
            | Format::Mkv
            | Format::Avi
            | Format::Webm
            | Format::Wmv
            | Format::Flv
            | Format::ThreeGp
            | Format::Ts => Category::Video,

            Format::Zip | Format::TarGz | Format::SevenZ => Category::Archive,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let cleaned = ext.trim().trim_start_matches('.').to_lowercase();
        match cleaned.as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" | "jpe" | "jfif" => Some(Format::Jpg),
            "webp" => Some(Format::Webp),
            "avif" => Some(Format::Avif),
            "heic" | "heif" => Some(Format::Heic),
            "tiff" | "tif" => Some(Format::Tiff),
            "bmp" | "dib" => Some(Format::Bmp),
            "gif" => Some(Format::Gif),
            "ico" => Some(Format::Ico),
            "svg" => Some(Format::Svg),
            "pdf" => Some(Format::Pdf),
            "docx" => Some(Format::Docx),
            "odt" => Some(Format::Odt),
            "rtf" => Some(Format::Rtf),
            "txt" | "text" | "log" => Some(Format::Txt),
            "md" | "markdown" => Some(Format::Md),
            "html" | "htm" => Some(Format::Html),
            "xlsx" => Some(Format::Xlsx),
            "xls" => Some(Format::Xls),
            "ods" => Some(Format::Ods),
            "csv" => Some(Format::Csv),
            "tsv" => Some(Format::Tsv),
            "json" => Some(Format::Json),
            "yaml" | "yml" => Some(Format::Yaml),
            "toml" => Some(Format::Toml),
            "xml" => Some(Format::Xml),
            "pptx" => Some(Format::Pptx),
            "ppt" => Some(Format::Ppt),
            "odp" => Some(Format::Odp),
            "mp3" => Some(Format::Mp3),
            "wav" => Some(Format::Wav),
            "flac" => Some(Format::Flac),
            "aac" => Some(Format::Aac),
            "ogg" | "oga" => Some(Format::Ogg),
            "m4a" => Some(Format::M4a),
            "opus" => Some(Format::Opus),
            "aiff" | "aif" => Some(Format::Aiff),
            "wma" => Some(Format::Wma),
            "mp4" | "m4v" => Some(Format::Mp4),
            "mov" => Some(Format::Mov),
            "mkv" => Some(Format::Mkv),
            "avi" => Some(Format::Avi),
            "webm" => Some(Format::Webm),
            "wmv" => Some(Format::Wmv),
            "flv" => Some(Format::Flv),
            "3gp" | "3gpp" | "3g2" => Some(Format::ThreeGp),
            "ts" | "m2ts" => Some(Format::Ts),
            "zip" => Some(Format::Zip),
            "tar.gz" | "tgz" => Some(Format::TarGz),
            "7z" => Some(Format::SevenZ),
            _ => None,
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        let mime = mime.to_lowercase();
        match mime.as_str() {
            "image/png" => Some(Format::Png),
            "image/jpeg" => Some(Format::Jpg),
            "image/webp" => Some(Format::Webp),
            "image/avif" => Some(Format::Avif),
            "image/heic" | "image/heif" => Some(Format::Heic),
            "image/tiff" => Some(Format::Tiff),
            "image/bmp" | "image/x-ms-bmp" => Some(Format::Bmp),
            "image/gif" => Some(Format::Gif),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some(Format::Ico),
            "image/svg+xml" => Some(Format::Svg),
            "application/pdf" => Some(Format::Pdf),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(Format::Docx)
            }
            "text/plain" => Some(Format::Txt),
            "text/markdown" => Some(Format::Md),
            "text/html" => Some(Format::Html),
            "text/csv" => Some(Format::Csv),
            "application/json" => Some(Format::Json),
            "application/yaml" | "text/yaml" => Some(Format::Yaml),
            "application/xml" | "text/xml" => Some(Format::Xml),
            "audio/mpeg" | "audio/mp3" => Some(Format::Mp3),
            "audio/wav" | "audio/x-wav" | "audio/wave" => Some(Format::Wav),
            "audio/flac" | "audio/x-flac" => Some(Format::Flac),
            "audio/aac" | "audio/x-aac" => Some(Format::Aac),
            "audio/ogg" | "application/ogg" => Some(Format::Ogg),
            "audio/mp4" | "audio/x-m4a" => Some(Format::M4a),
            "audio/opus" => Some(Format::Opus),
            "audio/aiff" | "audio/x-aiff" => Some(Format::Aiff),
            "audio/x-ms-wma" => Some(Format::Wma),
            "video/mp4" => Some(Format::Mp4),
            "video/quicktime" => Some(Format::Mov),
            "video/x-matroska" => Some(Format::Mkv),
            "video/webm" => Some(Format::Webm),
            "video/x-msvideo" | "video/avi" => Some(Format::Avi),
            "video/x-ms-wmv" => Some(Format::Wmv),
            "video/x-flv" => Some(Format::Flv),
            "video/3gpp" | "video/3gpp2" => Some(Format::ThreeGp),
            "video/mp2t" => Some(Format::Ts),
            "application/zip" => Some(Format::Zip),
            _ => None,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}
