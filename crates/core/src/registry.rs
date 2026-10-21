use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cancel::CancelToken;
use crate::converter::{Availability, Converter};
use crate::converters::ffmpeg::FfmpegConverter;
use crate::converters::image::PureRustImageConverter;
use crate::detect::detect_format;
use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::Options;
use crate::path_utils::resolve_target_path;

pub struct Registry {
    converters: Vec<Arc<dyn Converter>>,
}

impl Default for Registry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(PureRustImageConverter));
        reg.register(Arc::new(FfmpegConverter::default()));
        reg.register(Arc::new(crate::converters::data::DataConverter));
        reg.register(Arc::new(crate::converters::data::TextConverter));
        reg.register(Arc::new(crate::converters::archive::ArchiveConverter));
        reg.register(Arc::new(crate::converters::render::SvgConverter));
        reg.register(Arc::new(crate::converters::render::PdfConverter));
        reg.register(Arc::new(crate::converters::render::PdfTextConverter));
        reg.register(Arc::new(
            crate::converters::pdf_document::PdfDocumentConverter,
        ));
        reg.register(Arc::new(crate::converters::render::AvifFfmpegConverter));
        reg.register(Arc::new(crate::converters::render::MagickImageConverter(
            Format::Avif,
        )));
        reg.register(Arc::new(crate::converters::render::MagickImageConverter(
            Format::Heic,
        )));
        reg.register(Arc::new(crate::converters::office::OfficeConverter));
        reg
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            converters: Vec::new(),
        }
    }

    pub fn register(&mut self, converter: Arc<dyn Converter>) {
        self.converters.push(converter);
    }

    pub(crate) fn declared_targets(&self, from: Format) -> Vec<Format> {
        let preferred = from.category();
        let mut targets: Vec<_> = self
            .converters
            .iter()
            .filter(|c| c.sources().contains(&from))
            .flat_map(|c| c.targets(from))
            .collect();
        targets.sort_unstable_by_key(|&f| target_affinity_key(f, preferred));
        targets.dedup();
        targets
    }

    fn available_target_map(&self, from: Format) -> HashMap<Format, Availability> {
        let mut target_map: HashMap<Format, Availability> = HashMap::new();

        for conv in &self.converters {
            if conv.sources().contains(&from) {
                let targets = conv.targets(from);
                if targets.is_empty()
                    || targets
                        .iter()
                        .all(|t| target_map.get(t).is_some_and(Availability::is_ok))
                {
                    continue;
                }
                let avail = conv.available();
                for target in targets {
                    // If we already have a target, keep the one that is available (Ok)
                    target_map
                        .entry(target)
                        .and_modify(|existing| {
                            if !existing.is_ok() && avail.is_ok() {
                                *existing = avail.clone();
                            }
                        })
                        .or_insert_with(|| avail.clone());
                }
            }
        }

        target_map
    }

    /// Find all target formats available from a source format, with their availability.
    pub fn available_targets(&self, from: Format) -> Vec<(Format, Availability)> {
        // Return sorted by category affinity and format priority
        let mut list: Vec<(Format, Availability)> =
            self.available_target_map(from).into_iter().collect();
        sort_targets_by_affinity(&mut list, from.category());
        list
    }

    /// For batch conversion: compute common targets supported across all input formats.
    pub fn batch_targets(&self, formats: &[Format]) -> Vec<(Format, Availability)> {
        if formats.is_empty() {
            return Vec::new();
        }

        let mut common = self.available_target_map(formats[0]);
        let mut seen = HashSet::from([formats[0]]);

        for &fmt in &formats[1..] {
            if common.is_empty() {
                break;
            }
            if !seen.insert(fmt) {
                continue;
            }
            let next_targets = self.available_target_map(fmt);

            common.retain(|target, avail| {
                if let Some(other_avail) = next_targets.get(target) {
                    if !other_avail.is_ok() {
                        *avail = other_avail.clone();
                    }
                    true
                } else {
                    false
                }
            });
        }

        let preferred_cat = formats[0].category();
        let mut list: Vec<(Format, Availability)> = common.into_iter().collect();
        sort_targets_by_affinity(&mut list, preferred_cat);
        list
    }

    /// Only select engines that are currently usable, including fallbacks.
    pub fn find_converter(&self, from: Format, to: Format) -> Option<Arc<dyn Converter>> {
        self.converters
            .iter()
            .find(|conv| {
                conv.sources().contains(&from)
                    && conv.targets(from).contains(&to)
                    && conv.available().is_ok()
            })
            .cloned()
    }

    pub fn convert(
        &self,
        input: &Path,
        target: Format,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<PathBuf> {
        cancel.check()?;
        let opts = opts.normalized();
        let from = detect_format(input)?;
        let mut missing_reason = None;
        let mut direct_conv = None;
        for conv in &self.converters {
            if conv.sources().contains(&from) && conv.targets(from).contains(&target) {
                match conv.available() {
                    Availability::Ok => {
                        direct_conv = Some(Arc::clone(conv));
                        break;
                    }
                    Availability::Missing(reason) if missing_reason.is_none() => {
                        missing_reason = Some(reason);
                    }
                    Availability::Missing(_) => {}
                }
            }
        }
        let route = if let Some(conv) = direct_conv {
            vec![(target, conv)]
        } else if let Some(reason) = missing_reason {
            return Err(VertexError::EngineMissing(reason));
        } else if let Some(path) = self.find_path(from, target) {
            path
        } else {
            return Err(VertexError::NoConversionRoute {
                from: from.to_string(),
                to: target.to_string(),
            });
        };
        let mut destination = resolve_target_path(input, target, &opts)?;
        let mut staged = crate::path_utils::stage_output(&destination)?;
        let intermediate = (route.len() > 1)
            .then(tempfile::tempdir)
            .transpose()
            .map_err(|source| VertexError::Io {
                path: std::env::temp_dir(),
                source,
            })?;
        let mut current = input.to_path_buf();
        let mut current_format = from;
        let count = route.len();
        for (index, (format, conv)) in route.into_iter().enumerate() {
            cancel.check()?;
            let output = if index + 1 == count {
                staged.to_path_buf()
            } else {
                intermediate
                    .as_ref()
                    .expect("multi-hop workspace")
                    .path()
                    .join(format!("step-{index}.{}", format.extension()))
            };
            conv.convert_detected(
                crate::converter::DetectedInput {
                    path: &current,
                    format: current_format,
                },
                &output,
                &opts,
                &|p| progress((index as f32 + p.clamp(0.0, 1.0)) / count as f32),
                cancel,
            )?;
            current = output;
            current_format = format;
        }
        cancel.check()?;
        // No empty placeholders or global lock: atomically publish the completed
        // file, retrying a name collision without re-encoding.
        loop {
            crate::path_utils::ensure_distinct(input, &destination)?;
            let result = if matches!(
                opts.collision_policy,
                crate::options::CollisionPolicy::Overwrite
            ) {
                staged.persist(&destination)
            } else {
                staged.persist_noclobber(&destination)
            };
            match result {
                Ok(()) => return Ok(destination),
                Err(error)
                    if error.error.kind() == std::io::ErrorKind::AlreadyExists
                        && matches!(
                            opts.collision_policy,
                            crate::options::CollisionPolicy::RenameWithSuffix
                        ) =>
                {
                    staged = error.path;
                    cancel.check()?;
                    destination = resolve_target_path(input, target, &opts)?;
                }
                Err(error) => {
                    return Err(VertexError::Io {
                        path: destination,
                        source: error.error,
                    })
                }
            }
        }
    }

    /// BFS stores predecessor edges rather than cloning a path per visit.
    fn find_path(&self, from: Format, to: Format) -> Option<Vec<(Format, Arc<dyn Converter>)>> {
        if from == to {
            return None;
        }
        let mut queue = VecDeque::from([from]);
        let mut predecessors: HashMap<Format, (Format, Arc<dyn Converter>)> = HashMap::new();
        let engines: Vec<_> = self
            .converters
            .iter()
            .filter(|c| c.available().is_ok())
            .collect();
        while let Some(current) = queue.pop_front() {
            for conv in &engines {
                if !conv.sources().contains(&current) {
                    continue;
                }
                for next in conv.targets(current) {
                    if next == from {
                        continue;
                    }
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        predecessors.entry(next)
                    {
                        entry.insert((current, Arc::clone(conv)));
                        if next == to {
                            let mut path = Vec::new();
                            let mut at = to;
                            while let Some((previous, edge_conv)) = predecessors.remove(&at) {
                                path.push((at, edge_conv));
                                if previous == from {
                                    break;
                                }
                                at = previous;
                            }
                            path.reverse();
                            return Some(path);
                        }
                        queue.push_back(next);
                    }
                }
            }
        }
        None
    }
}

fn target_affinity_key(
    format: Format,
    preferred: crate::format::Category,
) -> (u8, usize, &'static str) {
    (
        if format.category() == preferred {
            0
        } else if format.category() == crate::format::Category::Audio {
            1
        } else {
            2
        },
        format_priority(format),
        format.label(),
    )
}

fn sort_targets_by_affinity(
    targets: &mut [(Format, Availability)],
    preferred: crate::format::Category,
) {
    targets.sort_unstable_by_key(|(format, _)| target_affinity_key(*format, preferred));
}

fn format_priority(fmt: Format) -> usize {
    match fmt {
        // Video priorities (most standard and popular first)
        Format::Mp4 => 1,
        Format::Webm => 2,
        Format::Mov => 3,
        Format::Mkv => 4,
        Format::Avi => 5,
        Format::Gif => 6,
        Format::Wmv => 7,
        Format::Flv => 8,
        Format::Ts => 9,
        Format::ThreeGp => 10,

        // Audio priorities
        Format::Mp3 => 1,
        Format::Wav => 2,
        Format::Flac => 3,
        Format::Aac => 4,
        Format::M4a => 5,
        Format::Ogg => 6,
        Format::Opus => 7,
        Format::Aiff => 8,
        Format::Wma => 9,

        // Image priorities
        Format::Png => 1,
        Format::Jpg => 2,
        Format::Webp => 3,
        Format::Avif => 4,
        Format::Bmp => 5,
        Format::Ico => 6,
        Format::Tiff => 7,
        Format::Svg => 8,
        Format::Heic => 9,

        // Document priorities
        Format::Pdf => 1,
        Format::Docx => 2,
        Format::Txt => 3,
        Format::Md => 4,
        Format::Html => 5,
        Format::Rtf => 6,
        Format::Odt => 7,
        Format::Doc => 8,
        Format::Fodt => 9,
        Format::Dotx => 10,
        Format::Ott => 11,

        // Spreadsheets
        Format::Xlsx => 1,
        Format::Csv => 2,
        Format::Json => 3,
        Format::Tsv => 4,
        Format::Xls => 5,
        Format::Ods => 6,
        Format::Yaml => 7,
        Format::Toml => 8,
        Format::Xml => 9,

        _ => 50,
    }
}
