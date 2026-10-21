use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter};
use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::Options;
use image::codecs::{gif::GifEncoder, jpeg::JpegEncoder, webp::WebPEncoder};
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, RgbImage};
use std::io::Cursor;
use std::path::Path;

const IMAGE_SOURCES: &[Format] = &[
    Format::Png,
    Format::Jpg,
    Format::Webp,
    Format::Bmp,
    Format::Ico,
    Format::Tiff,
    Format::Gif,
];

const IMAGE_TARGETS: &[Format] = &[
    Format::Png,
    Format::Jpg,
    Format::Webp,
    Format::Avif,
    Format::Bmp,
    Format::Ico,
    Format::Tiff,
    Format::Gif,
];

#[derive(Default)]
pub struct PureRustImageConverter;

#[derive(Default)]
pub(super) struct Metadata {
    exif: Option<Vec<u8>>,
    icc: Option<Vec<u8>>,
}
impl Metadata {
    fn apply(&mut self, encoder: &mut impl ImageEncoder) -> image::ImageResult<()> {
        if let Some(icc) = self.icc.take() {
            encoder
                .set_icc_profile(icc)
                .map_err(image::ImageError::Unsupported)?;
        }
        if let Some(exif) = self.exif.take() {
            encoder
                .set_exif_metadata(exif)
                .map_err(image::ImageError::Unsupported)?;
        }
        Ok(())
    }
}

fn flatten_on_background(img: DynamicImage, background: [u8; 3]) -> RgbImage {
    if !img.color().has_alpha() {
        return img.into_rgb8();
    }
    let rgba = img.into_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    let [bg0, bg1, bg2] = background.map(u16::from);
    let mut raw_rgb = vec![0u8; (width as usize) * (height as usize) * 3];
    for (&[r, g, b, a], out) in rgba
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(raw_rgb.as_chunks_mut::<3>().0)
    {
        let alpha = u16::from(a);
        let inv = 255 - alpha;
        *out = [
            ((u16::from(r) * alpha + bg0 * inv + 127) / 255) as u8,
            ((u16::from(g) * alpha + bg1 * inv + 127) / 255) as u8,
            ((u16::from(b) * alpha + bg2 * inv + 127) / 255) as u8,
        ];
    }
    RgbImage::from_raw(width, height, raw_rgb).expect("RGB buffer matches image dimensions")
}

/// SIMD dispatch and alpha-aware convolution preserve the source bit depth
/// without expanding an 8-bit image to four f32 channels.
fn resize_with_alpha(img: DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
    let mut output = DynamicImage::new(width, height, img.color());
    fast_image_resize::Resizer::new()
        .resize(
            &img,
            &mut output,
            &fast_image_resize::ResizeOptions::new().resize_alg(
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
            ),
        )
        .map_err(|e| VertexError::Encoding(format!("Image resize failed: {e}")))?;
    Ok(output)
}

impl Converter for PureRustImageConverter {
    fn name(&self) -> &'static str {
        "PureRustImageConverter"
    }
    fn sources(&self) -> &[Format] {
        IMAGE_SOURCES
    }
    fn targets(&self, _from: Format) -> Vec<Format> {
        IMAGE_TARGETS.to_vec()
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
        crate::path_utils::ensure_distinct(input, output)?;
        let opts = opts.normalized();
        let target = Format::from_path_extension(output)
            .ok_or_else(|| VertexError::UnrecognizedFormat(output.to_path_buf()))?;
        if !IMAGE_TARGETS.contains(&target) {
            return Err(VertexError::NoConversionRoute {
                from: "IMAGE".into(),
                to: target.to_string(),
            });
        }
        progress(0.1);
        let bytes = std::fs::read(input).map_err(|source| VertexError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        let reader = image::ImageReader::new(Cursor::new(bytes.as_slice()))
            .with_guessed_format()
            .map_err(|source| VertexError::Io {
                path: input.to_path_buf(),
                source,
            })?;
        let mut decoder = reader.into_decoder()?;
        let orientation = decoder.orientation()?;
        let mut metadata = Metadata::default();
        if !opts.strip_metadata {
            if matches!(target, Format::Jpg | Format::Png | Format::Webp) {
                metadata.exif = decoder.exif_metadata()?;
                if let Some(exif) = &mut metadata.exif {
                    let _ = image::metadata::Orientation::remove_from_exif_chunk(exif);
                }
            }
            if matches!(
                target,
                Format::Jpg | Format::Png | Format::Webp | Format::Tiff
            ) {
                metadata.icc = decoder.icc_profile()?;
            }
        }
        let mut img = DynamicImage::from_decoder(decoder)?;
        img.apply_orientation(orientation);
        encode_image(img, input, output, &opts, metadata, progress, cancel)
    }
}

/// Share the raster pipeline with SVG without an intermediate PNG encode,
/// filesystem round trip, and decode. Callers pass normalized options.
pub(super) fn encode_image(
    mut img: DynamicImage,
    input: &Path,
    output: &Path,
    opts: &Options,
    mut metadata: Metadata,
    progress: &dyn Fn(f32),
    cancel: &CancelToken,
) -> Result<()> {
    let target = Format::from_path_extension(output)
        .ok_or_else(|| VertexError::UnrecognizedFormat(output.to_path_buf()))?;
    let io = |source| VertexError::Io {
        path: output.to_path_buf(),
        source,
    };
    cancel.check()?;
    progress(0.35);

    let cap = if target == Format::Ico { 256 } else { u32::MAX };
    let max_w = opts.max_width.unwrap_or(u32::MAX).min(cap);
    let max_h = opts.max_height.unwrap_or(u32::MAX).min(cap);
    let ratio = (f64::from(max_w) / f64::from(img.width()))
        .min(f64::from(max_h) / f64::from(img.height()))
        .min(1.0);
    if ratio < 1.0 {
        let w = (f64::from(img.width()) * ratio).round().max(1.0) as u32;
        let h = (f64::from(img.height()) * ratio).round().max(1.0) as u32;
        img = resize_with_alpha(img, w.min(max_w), h.min(max_h))?;
    }
    cancel.check()?;
    progress(0.55);
    let mut buf = Vec::with_capacity(256 * 1024);
    match target {
        Format::Jpg => {
            let rgb = flatten_on_background(img, opts.jpeg_background);
            let mut encoder = JpegEncoder::new_with_quality(&mut buf, opts.quality);
            metadata.apply(&mut encoder)?;
            encoder.encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )?;
        }
        Format::Webp => {
            let img = if img.color().has_alpha() {
                DynamicImage::ImageRgba8(img.into_rgba8())
            } else {
                DynamicImage::ImageRgb8(img.into_rgb8())
            };
            let mut encoder = WebPEncoder::new_lossless(&mut buf);
            metadata.apply(&mut encoder)?;
            encoder.encode(
                img.as_bytes(),
                img.width(),
                img.height(),
                img.color().into(),
            )?;
        }
        Format::Avif => {
            use rgb::FromSlice;
            let encoder = ravif::Encoder::new()
                .with_quality(f32::from(opts.quality))
                .with_alpha_quality(100.0)
                .with_speed(opts.avif_speed);
            let result = if img.color().has_alpha() {
                let rgba = img.into_rgba8();
                encoder.encode_rgba(ravif::Img::new(
                    rgba.as_raw().as_rgba(),
                    rgba.width() as usize,
                    rgba.height() as usize,
                ))
            } else {
                let rgb = img.into_rgb8();
                encoder.encode_rgb(ravif::Img::new(
                    rgb.as_raw().as_rgb(),
                    rgb.width() as usize,
                    rgb.height() as usize,
                ))
            }
            .map_err(|e| VertexError::Encoding(format!("AVIF encode failed: {e}")))?;
            buf = result.avif_file;
        }
        Format::Gif => {
            let has_alpha = img.color().has_alpha();
            let mut rgba = img.into_rgba8();
            if has_alpha {
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    if pixel[3] < opts.gif_alpha_threshold {
                        *pixel = [0; 4];
                    } else {
                        pixel[3] = 255;
                    }
                }
            }
            GifEncoder::new_with_speed(&mut buf, 10).encode_frame(image::Frame::new(rgba))?;
        }
        Format::Ico => DynamicImage::ImageRgba8(img.into_rgba8())
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Ico)?,
        Format::Bmp => {
            let img = if img.color().has_alpha() {
                DynamicImage::ImageRgba8(img.into_rgba8())
            } else {
                DynamicImage::ImageRgb8(img.into_rgb8())
            };
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Bmp)?;
        }
        Format::Png => {
            let img = match img {
                DynamicImage::ImageRgb32F(_) => DynamicImage::ImageRgb16(img.into_rgb16()),
                DynamicImage::ImageRgba32F(_) => DynamicImage::ImageRgba16(img.into_rgba16()),
                image => image,
            };
            let mut encoder = image::codecs::png::PngEncoder::new(&mut buf);
            metadata.apply(&mut encoder)?;
            img.write_with_encoder(encoder)?;
        }
        Format::Tiff => {
            let mut cursor = Cursor::new(&mut buf);
            let mut encoder = image::codecs::tiff::TiffEncoder::new(&mut cursor);
            metadata.apply(&mut encoder)?;
            img.write_with_encoder(encoder)?;
        }
        _ => {
            return Err(VertexError::NoConversionRoute {
                from: "IMAGE".into(),
                to: target.to_string(),
            })
        }
    }
    let staged = crate::path_utils::stage_output(output)?;
    std::fs::write(&staged, &buf).map_err(io)?;
    cancel.check()?;
    crate::path_utils::ensure_distinct(input, output)?;
    crate::path_utils::publish_exact(staged, output)?;
    progress(1.0);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn jpeg_flatten_preserves_rgb_and_composites_alpha() {
        let rgb = RgbImage::from_pixel(2, 1, Rgb([17, 89, 201]));
        assert_eq!(
            flatten_on_background(DynamicImage::ImageRgb8(rgb.clone()), [255; 3]),
            rgb
        );
        let rgba = image::RgbaImage::from_fn(3, 1, |x, _| {
            image::Rgba([20, 100, 200, [0, 128, 255][x as usize]])
        });
        let result = flatten_on_background(DynamicImage::ImageRgba8(rgba), [255; 3]);
        assert_eq!(result.get_pixel(0, 0).0, [255; 3]);
        assert_eq!(result.get_pixel(1, 0).0, [137, 177, 227]);
        assert_eq!(result.get_pixel(2, 0).0, [20, 100, 200]);
    }
}
