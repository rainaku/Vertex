use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, VertexError>;

#[derive(Error, Debug)]
pub enum VertexError {
    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Unrecognized or unsupported file format for: {0}")]
    UnrecognizedFormat(PathBuf),

    #[error("No conversion route found from {from} to {to}")]
    NoConversionRoute { from: String, to: String },

    #[error("Conversion engine missing: {0}")]
    EngineMissing(String),

    #[error("Operation was cancelled by user")]
    Cancelled,

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Invalid conversion options: {0}")]
    InvalidOptions(String),

    #[error("Internal conversion error: {0}")]
    Internal(String),
}
