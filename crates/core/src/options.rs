use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    /// Append numeric suffix: "image (1).png", "image (2).png"
    RenameWithSuffix,
    /// Return an error if file already exists
    FailIfExists,
    /// Overwrite existing file (disabled by default)
    Overwrite,
}

impl Default for CollisionPolicy {
    fn default() -> Self {
        CollisionPolicy::RenameWithSuffix
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Encoding quality 1-100 (for JPG, WEBP, AVIF). Default 85.
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
        }
    }
}
