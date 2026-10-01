use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    /// Append numeric suffix: "image (1).png", "image (2).png"
    #[default]
    RenameWithSuffix,
    /// Return an error if file already exists
    FailIfExists,
    /// Overwrite existing file (disabled by default)
    Overwrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Encoding quality 1-100 (for JPG and AVIF). WebP is lossless. Default 85.
    pub quality: u8,
    /// Rendering DPI for vector/PDF to raster. Default 200.
    pub dpi: u32,
    /// Optional max width for downscaling.
    pub max_width: Option<u32>,
    /// Optional max height for downscaling.
    pub max_height: Option<u32>,
    /// Strip EXIF / ICC / metadata. Default true.
    pub strip_metadata: bool,
    /// Custom output directory. If None, saves next to input file.
    pub output_dir: Option<PathBuf>,
    /// Naming collision policy.
    pub collision_policy: CollisionPolicy,
    /// RGB matte used when exporting to JPEG, which cannot store alpha.
    pub jpeg_background: [u8; 3],
    /// Pixels below this alpha value become transparent in GIF.
    pub gif_alpha_threshold: u8,
    /// AVIF encoding speed, 1 (slowest) through 10 (fastest).
    pub avif_speed: u8,
    pub video_codec: VideoCodec,
    pub video_preset: VideoPreset,
    /// Constant-quality value for H.264/H.265. Lower means higher quality.
    pub video_crf: u8,
    pub video_audio_kbps: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    #[default]
    H264,
    H265,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VideoPreset {
    Fast,
    #[default]
    Medium,
    Slow,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quality: 85,
            dpi: 200,
            max_width: None,
            max_height: None,
            strip_metadata: true,
            output_dir: None,
            collision_policy: CollisionPolicy::default(),
            jpeg_background: [255, 255, 255],
            gif_alpha_threshold: 128,
            avif_speed: 6,
            video_codec: VideoCodec::H264,
            video_preset: VideoPreset::Medium,
            video_crf: 23,
            video_audio_kbps: 128,
        }
    }
}
