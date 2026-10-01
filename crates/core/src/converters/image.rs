use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageFormat};
use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter};
use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::Options;

pub struct PureRustImageConverter {
    supported_sources: Vec<Format>,
    supported_targets: Vec<Format>,
}

impl Default for PureRustImageConverter {
    fn default() -> Self {
        Self {
            supported_sources: vec![
                Format::Png,
                Format::Jpg,
                Format::Webp,
                Format::Bmp,
                Format::Ico,
                Format::Tiff,
                Format::Gif,
            ],
            supported_targets: vec![
                Format::Png,
                Format::Jpg,
                Format::Webp,
                Format::Avif,
                Format::Bmp,
                Format::Ico,
                Format::Tiff,
                Format::Gif,
            ],
        }
    }
}

impl Converter for PureRustImageConverter {
    fn name(&self) -> &'static str {
        "PureRustImageConverter"
    }

    fn sources(&self) -> &[Format] {
        &self.supported_sources
    }

    fn targets(&self, _from: Format) -> Vec<Format> {
        self.supported_targets.clone()
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
        cancel.check()?;
        progress(0.1);

        // 1. Decode image using pure Rust `image` crate
        let mut img = image::open(input).map_err(VertexError::Image)?;

        cancel.check()?;
        progress(0.35);

        // 2. Resize if specified in Options
        if let (Some(max_w), Some(max_h)) = (opts.max_width, opts.max_height) {
            if img.width() > max_w || img.height() > max_h {
                img = img.resize(max_w, max_h, FilterType::Lanczos3);
            }
        } else if let Some(max_w) = opts.max_width {
            if img.width() > max_w {
                let ratio = max_w as f32 / img.width() as f32;
                let new_h = (img.height() as f32 * ratio).round() as u32;
                img = img.resize(max_w, new_h.max(1), FilterType::Lanczos3);
            }
        } else if let Some(max_h) = opts.max_height {
            if img.height() > max_h {
                let ratio = max_h as f32 / img.height() as f32;
                let new_w = (img.width() as f32 * ratio).round() as u32;
                img = img.resize(new_w.max(1), max_h, FilterType::Lanczos3);
            }
        }

        cancel.check()?;
        progress(0.55);

        // Detect target format from output path extension
        let target_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let target_fmt = Format::from_extension(target_ext).unwrap_or(Format::Png);

        // 3. Encode to target format
        match target_fmt {
            Format::Png => {
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                img.write_to(&mut writer, ImageFormat::Png)
                    .map_err(VertexError::Image)?;
            }
            Format::Jpg => {
                let rgb = img.to_rgb8();
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                let mut encoder = JpegEncoder::new_with_quality(&mut writer, opts.quality.clamp(1, 100));
                encoder
                    .encode(
                        rgb.as_raw(),
                        rgb.width(),
                        rgb.height(),
                        ExtendedColorType::Rgb8,
                    )
                    .map_err(VertexError::Image)?;
            }
            Format::Webp => {
                let rgba = img.to_rgba8();
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                let encoder = WebPEncoder::new_lossless(&mut writer);
                encoder
                    .encode(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(VertexError::Image)?;
            }
            Format::Avif => {
                let rgba = img.to_rgba8();
                let width = rgba.width() as usize;
                let height = rgba.height() as usize;
                let raw_bytes = rgba.as_raw();

                let mut pixels = Vec::with_capacity(width * height);
                for chunk in raw_bytes.chunks_exact(4) {
                    pixels.push(rgb::RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                }

                let encoder = ravif::Encoder::new()
                    .with_quality(opts.quality as f32)
                    .with_speed(6);

                let res = encoder
                    .encode_rgba(ravif::Img::new(&pixels, width, height))
                    .map_err(|e| VertexError::Encoding(format!("AVIF encode failed: {e}")))?;

                std::fs::write(output, res.avif_file).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
            }
            Format::Bmp => {
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                img.write_to(&mut writer, ImageFormat::Bmp)
                    .map_err(VertexError::Image)?;
            }
            Format::Ico => {
                // ICO typically requires max 256x256 dimensions
                let ico_img = if img.width() > 256 || img.height() > 256 {
                    img.resize(256, 256, FilterType::Lanczos3)
                } else {
                    img.clone()
                };
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                ico_img
                    .write_to(&mut writer, ImageFormat::Ico)
                    .map_err(VertexError::Image)?;
            }
            Format::Tiff => {
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                img.write_to(&mut writer, ImageFormat::Tiff)
                    .map_err(VertexError::Image)?;
            }
            Format::Gif => {
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                img.write_to(&mut writer, ImageFormat::Gif)
                    .map_err(VertexError::Image)?;
            }
            other => {
                return Err(VertexError::NoConversionRoute {
                    from: "IMAGE".to_string(),
                    to: other.to_string(),
                });
            }
        }

        cancel.check()?;
        progress(1.0);
        Ok(())
    }
}
