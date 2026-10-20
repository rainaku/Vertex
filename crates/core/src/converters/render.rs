use super::external::{find_tool, io_error, run_tool, workspace};
use super::image::PureRustImageConverter;
use crate::{Availability, CancelToken, Converter, Format, Options, Result, VertexError};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const MAX_SVG_SIZE: u64 = 64 * 1024 * 1024;

pub const RASTER_TARGETS: &[Format] = &[
    Format::Png,
    Format::Jpg,
    Format::Webp,
    Format::Avif,
    Format::Bmp,
    Format::Ico,
    Format::Tiff,
    Format::Gif,
];

pub struct SvgConverter;
impl Converter for SvgConverter {
    fn name(&self) -> &'static str {
        "SvgConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Svg]
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        RASTER_TARGETS.to_vec()
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
        let file = std::fs::File::open(input).map_err(|e| io_error(input, e))?;
        let mut data = Vec::new();
        file.take(MAX_SVG_SIZE + 1)
            .read_to_end(&mut data)
            .map_err(|e| io_error(input, e))?;
        if data.len() as u64 > MAX_SVG_SIZE {
            return Err(VertexError::Encoding(
                "SVG input is limited to 64 MiB per conversion".into(),
            ));
        }
        let mut config = resvg::usvg::Options {
            dpi: opts.dpi as f32,
            ..Default::default()
        };
        // SVGs can embed data URLs; don't read arbitrary paths referenced by them.
        config.image_href_resolver.resolve_string = Box::new(|_, _| None);
        static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
            std::sync::OnceLock::new();
        config.fontdb = FONTS
            .get_or_init(|| {
                let mut db = resvg::usvg::fontdb::Database::new();
                db.load_system_fonts();
                std::sync::Arc::new(db)
            })
            .clone();
        let tree = resvg::usvg::Tree::from_data(&data, &config)
            .map_err(|e| VertexError::Encoding(e.to_string()))?;
        let size = tree.size();
        let scale = (opts.max_width.unwrap_or(32768) as f32 / size.width())
            .min(opts.max_height.unwrap_or(32768) as f32 / size.height())
            .min(1.0);
        let width = (size.width() * scale).round().max(1.0) as u32;
        let height = (size.height() * scale).round().max(1.0) as u32;
        if u64::from(width) * u64::from(height) > 100_000_000 {
            return Err(VertexError::InvalidOptions(
                "SVG raster exceeds 100 megapixels; reduce DPI or maximum dimensions".into(),
            ));
        }
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| VertexError::Encoding("Could not allocate SVG raster".into()))?;
        cancel.check()?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let temp = workspace()?;
        let png = temp.path().join("render.png");
        pixmap
            .save_png(&png)
            .map_err(|e| VertexError::Encoding(e.to_string()))?;
        PureRustImageConverter::default().convert(
            &png,
            output,
            &opts,
            &|p| progress(0.5 + p * 0.5),
            cancel,
        )
    }
}

pub struct PdfConverter;
impl Converter for PdfConverter {
    fn name(&self) -> &'static str {
        "PdfConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Pdf]
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        RASTER_TARGETS.to_vec()
    }
    fn available(&self) -> Availability {
        if find_tool(
            "pdftoppm",
            &["Poppler/Library/bin", "/usr/bin", "/opt/homebrew/bin"],
        )
        .is_some()
        {
            Availability::Ok
        } else {
            Availability::Missing("PDF → ảnh cần Poppler (pdftoppm) trong PATH hoặc thư mục bin của Vertex. / PDF rendering requires Poppler (pdftoppm).".into())
        }
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
        let binary = find_tool(
            "pdftoppm",
            &["Poppler/Library/bin", "/usr/bin", "/opt/homebrew/bin"],
        )
        .ok_or_else(|| VertexError::EngineMissing("Poppler (pdftoppm)".into()))?;
        let temp = workspace()?;
        let prefix = temp.path().join("page");
        let source = std::fs::canonicalize(input).map_err(|e| io_error(input, e))?;
        let mut command = Command::new(binary);
        command.args([
            "-f",
            "1",
            "-l",
            "1",
            "-singlefile",
            "-r",
            &opts.dpi.to_string(),
            "-png",
        ]);
        command
            .arg(crate::path_utils::external_path(&source))
            .arg(&prefix);
        run_tool(&mut command, cancel, Duration::from_secs(300))?;
        PureRustImageConverter::default().convert(
            &prefix.with_extension("png"),
            output,
            &opts,
            &|p| progress(0.5 + p * 0.5),
            cancel,
        )
    }
}

/// ImageMagick is used only for decoding formats absent from the Rust decoder.
/// Every output then uses the same image encoder and option handling as PNG/JPG.
pub struct MagickImageConverter(pub Format);
impl MagickImageConverter {
    fn binary(&self) -> Option<std::path::PathBuf> {
        find_tool("magick", &["ImageMagick", "/usr/bin", "/opt/homebrew/bin"])
    }
    fn decoder_available(&self) -> bool {
        type Probe = std::sync::Mutex<Option<(std::time::Instant, String)>>;
        static CACHE: std::sync::OnceLock<Probe> = std::sync::OnceLock::new();
        let mut cache = CACHE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if !cache
            .as_ref()
            .is_some_and(|(time, _)| time.elapsed() < Duration::from_secs(30))
        {
            let text = self
                .binary()
                .and_then(|binary| {
                    run_tool(
                        Command::new(binary).args(["-list", "format"]),
                        &CancelToken::new(),
                        Duration::from_secs(10),
                    )
                    .ok()
                })
                .unwrap_or_default();
            *cache = Some((std::time::Instant::now(), text));
        }
        cache.as_ref().is_some_and(|(_, text)| {
            text.lines().any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                fields.len() >= 3
                    && fields[0].trim_end_matches('*') == self.0.label()
                    && fields[2].contains('r')
            })
        })
    }
}
impl Converter for MagickImageConverter {
    fn name(&self) -> &'static str {
        "MagickImageConverter"
    }
    fn sources(&self) -> &[Format] {
        std::slice::from_ref(&self.0)
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        RASTER_TARGETS.to_vec()
    }
    fn available(&self) -> Availability {
        if self.decoder_available() {
            Availability::Ok
        } else {
            Availability::Missing(format!("{} cần ImageMagick với decoder {} (libheif). / Install ImageMagick with {} decoding support.", self.0, self.0, self.0))
        }
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
        let binary = self
            .binary()
            .ok_or_else(|| VertexError::EngineMissing("ImageMagick".into()))?;
        let temp = workspace()?;
        // Copy to a controlled basename: ImageMagick treats [] and colons as syntax.
        let source = temp.path().join(format!("source.{}", self.0.extension()));
        std::fs::copy(input, &source).map_err(|e| io_error(input, e))?;
        let png = temp.path().join("decoded.png");
        run_tool(
            Command::new(binary)
                .current_dir(temp.path())
                .args(["-limit", "memory", "256MiB", "-limit", "map", "512MiB"])
                .arg(format!("source.{}[0]", self.0.extension()))
                .arg("decoded.png"),
            cancel,
            Duration::from_secs(300),
        )?;
        PureRustImageConverter::default().convert(
            &png,
            output,
            opts,
            &|p| progress(0.5 + p * 0.5),
            cancel,
        )
    }
}

pub struct AvifFfmpegConverter;
impl Converter for AvifFfmpegConverter {
    fn name(&self) -> &'static str {
        "AvifFfmpegConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Avif]
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        RASTER_TARGETS.to_vec()
    }
    fn available(&self) -> Availability {
        if super::ffmpeg::find_ffmpeg().is_some() {
            Availability::Ok
        } else {
            Availability::Missing("AVIF đầu vào cần FFmpeg hoặc ImageMagick. / AVIF decoding requires FFmpeg or ImageMagick.".into())
        }
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
        let binary = super::ffmpeg::find_ffmpeg()
            .ok_or_else(|| VertexError::EngineMissing("FFmpeg".into()))?;
        let temp = workspace()?;
        let png = temp.path().join("decoded.png");
        let source = std::fs::canonicalize(input).map_err(|e| io_error(input, e))?;
        run_tool(
            Command::new(binary)
                .args(["-nostdin", "-v", "error", "-y", "-i"])
                .arg(crate::path_utils::external_path(&source))
                .args(["-frames:v", "1"])
                .arg(&png),
            cancel,
            Duration::from_secs(300),
        )?;
        PureRustImageConverter::default().convert(
            &png,
            output,
            opts,
            &|p| progress(0.5 + p * 0.5),
            cancel,
        )
    }
}

pub struct PdfTextConverter;
impl Converter for PdfTextConverter {
    fn name(&self) -> &'static str {
        "PdfTextConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Pdf]
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        vec![Format::Txt]
    }
    fn available(&self) -> Availability {
        if find_tool(
            "pdftotext",
            &["Poppler/Library/bin", "/usr/bin", "/opt/homebrew/bin"],
        )
        .is_some()
        {
            Availability::Ok
        } else {
            Availability::Missing("PDF → TXT cần Poppler (pdftotext). / PDF text extraction requires Poppler (pdftotext).".into())
        }
    }
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input, output)?;
        let binary = find_tool(
            "pdftotext",
            &["Poppler/Library/bin", "/usr/bin", "/opt/homebrew/bin"],
        )
        .ok_or_else(|| VertexError::EngineMissing("Poppler (pdftotext)".into()))?;
        let staged = crate::path_utils::stage_output(output)?;
        let source = std::fs::canonicalize(input).map_err(|e| io_error(input, e))?;
        run_tool(
            Command::new(binary)
                .args(["-enc", "UTF-8", "-layout"])
                .arg(crate::path_utils::external_path(&source))
                .arg(staged.as_os_str()),
            cancel,
            Duration::from_secs(300),
        )?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}
