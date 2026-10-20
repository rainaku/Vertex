//! Exported conversion contract. The browser snapshot is verified in tests.
use crate::{Category, Format, Options, Registry};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct FormatDefinition {
    pub format: Format,
    pub label: &'static str,
    pub extension: &'static str,
    pub extensions: Vec<&'static str>,
    pub category: Category,
    pub targets: Vec<Format>,
    pub notices: BTreeMap<&'static str, ConversionNotice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct ConversionNotice {
    pub vi: String,
    pub en: String,
}

pub fn conversion_notice(from: Format, to: Format) -> Option<ConversionNotice> {
    let (vi, en) = if from == Format::Pdf && to.category() == Category::Image {
        (
            "Xuất trang đầu của PDF theo DPI đã chọn.",
            "Renders the first PDF page at the selected DPI.",
        )
    } else if from == Format::Pdf && to == Format::Txt {
        (
            "Trích xuất lớp văn bản của toàn bộ PDF; không nhận dạng OCR.",
            "Extracts the PDF text layer from all pages; no OCR.",
        )
    } else if matches!(from, Format::Xlsx | Format::Xls | Format::Ods)
        && matches!(to, Format::Csv | Format::Tsv)
    {
        (
            "Xuất sheet đầu tiên; công thức được xuất thành giá trị.",
            "Exports the first sheet; formulas are exported as values.",
        )
    } else if matches!(to, Format::Csv | Format::Tsv)
        && matches!(
            from,
            Format::Json | Format::Yaml | Format::Toml | Format::Xml
        )
    {
        ("Dữ liệu cần là mảng các đối tượng có ô vô hướng; không tự làm phẳng dữ liệu lồng nhau.", "Requires an array of objects with scalar cells; nested data is not flattened.")
    } else if from == Format::Xml || to == Format::Xml {
        ("XML dùng @ cho thuộc tính, $text cho nội dung và mảng cho phần tử lặp. Giá trị XML là chuỗi.", "XML maps attributes to @keys, text to $text and repeated elements to arrays. XML scalars are strings.")
    } else {
        return None;
    };
    Some(ConversionNotice {
        vi: vi.into(),
        en: en.into(),
    })
}

#[derive(Clone, Copy, Serialize)]
pub struct NumericLimit {
    pub min: u32,
    pub max: u32,
}
pub const QUALITY: NumericLimit = NumericLimit { min: 1, max: 100 };
pub const DPI: NumericLimit = NumericLimit { min: 72, max: 600 };
pub const DIMENSION: NumericLimit = NumericLimit { min: 1, max: 32768 };
pub const GIF_ALPHA: NumericLimit = NumericLimit { min: 1, max: 255 };
pub const AVIF_SPEED: NumericLimit = NumericLimit { min: 1, max: 10 };
pub const VIDEO_CRF: NumericLimit = NumericLimit { min: 18, max: 35 };
pub const VIDEO_AUDIO: NumericLimit = NumericLimit { min: 64, max: 320 };

#[derive(Serialize)]
pub struct ConversionContract {
    pub default_options: Options,
    pub limits: BTreeMap<&'static str, NumericLimit>,
    pub formats: Vec<FormatDefinition>,
}

impl Registry {
    pub fn source_formats(&self) -> Vec<FormatDefinition> {
        Format::ALL
            .iter()
            .copied()
            .filter_map(|format| {
                let targets = self.declared_targets(format);
                (!targets.is_empty()).then(|| FormatDefinition {
                    format,
                    label: format.label(),
                    extension: format.extension(),
                    extensions: format.extensions(),
                    category: format.category(),
                    notices: targets
                        .iter()
                        .filter_map(|target| {
                            conversion_notice(format, *target)
                                .map(|notice| (target.label(), notice))
                        })
                        .collect(),
                    targets,
                })
            })
            .collect()
    }
    pub fn contract(&self) -> ConversionContract {
        ConversionContract {
            default_options: Options::default(),
            limits: BTreeMap::from([
                ("quality", QUALITY),
                ("dpi", DPI),
                ("max_width", DIMENSION),
                ("max_height", DIMENSION),
                ("gif_alpha_threshold", GIF_ALPHA),
                ("avif_speed", AVIF_SPEED),
                ("video_crf", VIDEO_CRF),
                ("video_audio_kbps", VIDEO_AUDIO),
            ]),
            formats: self.source_formats(),
        }
    }
}
