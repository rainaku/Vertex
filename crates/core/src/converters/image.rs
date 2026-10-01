use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter};
use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::Options;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageFormat, Rgb, RgbImage};

pub struct PureRustImageConverter {
    supported_sources: Vec<Format>,
    supported_targets: Vec<Format>,
}

/// JPEG has no alpha channel. Dropping alpha exposes arbitrary RGB values
/// stored in transparent pixels (often black, sometimes colored noise).
fn flatten_on_background(img: &DynamicImage, background: [u8; 3]) -> RgbImage {
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let alpha = u32::from(pixel[3]);
        Rgb(std::array::from_fn(|channel| {
            ((u32::from(pixel[channel]) * alpha
                + u32::from(background[channel]) * (255 - alpha)
                + 127)
                / 255) as u8
        }))
    })
}

/// Resampling straight-alpha RGB leaks invisible pixel colors into the edge.
/// Filter premultiplied floating-point pixels, then restore straight alpha
/// for the encoders. This also covers the automatic ICO downscale.
fn resize_with_alpha(img: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    if !img.color().has_alpha() {
        return img.resize(width, height, FilterType::Lanczos3);
    }
    let mut premultiplied = img.to_rgba32f();
    for pixel in premultiplied.pixels_mut() {
        let alpha = pixel[3];
        for channel in &mut pixel.0[..3] {
            *channel *= alpha;
        }
    }
    let mut resized = DynamicImage::ImageRgba32F(premultiplied)
        .resize(width, height, FilterType::Lanczos3)
        .to_rgba32f();
    for pixel in resized.pixels_mut() {
        let alpha = pixel[3].clamp(0.0, 1.0);
        for channel in &mut pixel.0[..3] {
            *channel = if alpha > 0.0 {
                (*channel / alpha).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
        pixel[3] = alpha;
    }
    let resized = DynamicImage::ImageRgba32F(resized);
    match img.color() {
        image::ColorType::La16 | image::ColorType::Rgba16 => {
            DynamicImage::ImageRgba16(resized.to_rgba16())
        }
        image::ColorType::Rgba32F => resized,
        _ => DynamicImage::ImageRgba8(resized.to_rgba8()),
    }
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
                img = resize_with_alpha(&img, max_w.max(1), max_h.max(1));
            }
        } else if let Some(max_w) = opts.max_width {
            if img.width() > max_w {
                let ratio = max_w as f32 / img.width() as f32;
                let new_h = (img.height() as f32 * ratio).round() as u32;
                img = resize_with_alpha(&img, max_w.max(1), new_h.max(1));
            }
        } else if let Some(max_h) = opts.max_height {
            if img.height() > max_h {
                let ratio = max_h as f32 / img.height() as f32;
                let new_w = (img.width() as f32 * ratio).round() as u32;
                img = resize_with_alpha(&img, new_w.max(1), max_h.max(1));
            }
        }

        cancel.check()?;
        progress(0.55);

        // Detect target format from output path extension
        let target_ext = output.extension().and_then(|e| e.to_str()).unwrap_or("png");
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
                let rgb = flatten_on_background(&img, opts.jpeg_background);
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                let mut encoder =
                    JpegEncoder::new_with_quality(&mut writer, opts.quality.clamp(1, 100));
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
                for chunk in raw_bytes.as_chunks::<4>().0 {
                    pixels.push(rgb::RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                }

                let encoder = ravif::Encoder::new()
                    .with_quality(opts.quality.clamp(1, 100) as f32)
                    // Keep the transparency mask at full quality independently
                    // of the lossy color quality chosen by the user.
                    .with_alpha_quality(100.0)
                    .with_speed(opts.avif_speed.clamp(1, 10));

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
                // BMP's grayscale+alpha path drops alpha; normalize to RGBA
                // so the encoder writes the V4 header and explicit alpha mask.
                let bmp = if img.color().has_alpha() {
                    DynamicImage::ImageRgba8(img.to_rgba8())
                } else {
                    DynamicImage::ImageRgb8(img.to_rgb8())
                };
                bmp.write_to(&mut writer, ImageFormat::Bmp)
                    .map_err(VertexError::Image)?;
            }
            Format::Ico => {
                // ICO typically requires max 256x256 dimensions
                let ico_img = if img.width() > 256 || img.height() > 256 {
                    resize_with_alpha(&img, 256, 256)
                } else {
                    img.clone()
                };
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                DynamicImage::ImageRgba8(ico_img.to_rgba8())
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
                // GIF supports only binary transparency. The encoder treats
                // every nonzero alpha as opaque, exposing nearly invisible
                // PNG edge pixels. Choose the cutoff explicitly instead.
                let mut rgba = img.to_rgba8();
                for pixel in rgba.pixels_mut() {
                    if pixel[3] < opts.gif_alpha_threshold.max(1) {
                        pixel.0 = [0, 0, 0, 0];
                    } else {
                        pixel[3] = 255;
                    }
                }
                let file = File::create(output).map_err(|e| VertexError::Io {
                    path: output.to_path_buf(),
                    source: e,
                })?;
                let mut writer = BufWriter::new(file);
                DynamicImage::ImageRgba8(rgba)
                    .write_to(&mut writer, ImageFormat::Gif)
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
