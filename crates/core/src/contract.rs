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
    } else if from == Format::Pdf && to == Format::Docx {
        (
            "Word có thể sửa chữ và bảng; bố cục có thể thay đổi. PDF scan cần OCR trước; ảnh không tự thành chữ.",
            "Editable Word text and tables; layout may change. Scanned PDFs need OCR first; images do not become text.",
        )
    } else if from == Format::Pdf && to == Format::Txt {
        (
            "Trích xuất chữ toàn bộ PDF, giữ khoảng cách tương đối; mất font, ảnh và định dạng bảng. Không OCR.",
            "Extracts text from all PDF pages with approximate spacing; fonts, images and table formatting are lost. No OCR.",
        )
    } else if from.category() == Category::Document && to == Format::Txt {
        (
            "TXT chỉ giữ văn bản; không lưu font, màu, ảnh hoặc định dạng bảng.",
            "TXT retains plain text only; fonts, colors, images and table formatting are not preserved.",
        )
    } else if from.category() == Category::Document && to == Format::Pdf {
        (
            "Giữ bố cục bằng bộ xuất tài liệu; cần cài font gốc để hạn chế thay font và lệch trang.",
            "Exports document layout; install the original fonts to reduce font substitution and pagination changes.",
        )
    } else if from.category() == Category::Document
        && matches!(to, Format::Doc | Format::Docx | Format::Odt | Format::Rtf | Format::Html)
        && !matches!(from, Format::Txt | Format::Md)
    {
        (
            "Giữ định dạng trong khả năng của định dạng đích; font thiếu và bố cục phức tạp có thể thay đổi.",
            "Preserves formatting supported by the target; missing fonts and complex layouts may cause changes.",
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
