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
        reg.register(Arc::new(PureRustImageConverter::default()));
        reg.register(Arc::new(FfmpegConverter::default()));
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

    /// Find all target formats available from a source format, with their availability.
    pub fn available_targets(&self, from: Format) -> Vec<(Format, Availability)> {
        let mut target_map: HashMap<Format, Availability> = HashMap::new();

        for conv in &self.converters {
            if conv.sources().contains(&from) {
                let avail = conv.available();
                for target in conv.targets(from) {
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

        // Return sorted by category affinity and format priority
        let mut list: Vec<(Format, Availability)> = target_map.into_iter().collect();
        sort_targets_by_affinity(&mut list, from.category());
        list
    }

    /// For batch conversion: compute common targets supported across all input formats.
    pub fn batch_targets(&self, formats: &[Format]) -> Vec<(Format, Availability)> {
        if formats.is_empty() {
            return Vec::new();
        }

        let first_targets = self.available_targets(formats[0]);
        let mut common: HashMap<Format, Availability> = first_targets.into_iter().collect();

        for &fmt in &formats[1..] {
            let next_targets: HashMap<Format, Availability> =
                self.available_targets(fmt).into_iter().collect();

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

    /// Find direct converter for a single hop
    pub fn find_converter(&self, from: Format, to: Format) -> Option<Arc<dyn Converter>> {
        for conv in &self.converters {
            if conv.sources().contains(&from) && conv.targets(from).contains(&to) {
                return Some(conv.clone());
            }
        }
        None
    }

    /// Execute conversion from input file to target format
    pub fn convert(
        &self,
        input: &Path,
        target_format: Format,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<PathBuf> {
        let from_format = detect_format(input)?;
        let output_path = resolve_target_path(input, target_format, opts)?;

        // Find direct converter
        if let Some(conv) = self.find_converter(from_format, target_format) {
            match conv.available() {
                Availability::Ok => {
                    conv.convert(input, &output_path, opts, progress, cancel)?;
                    return Ok(output_path);
                }
                Availability::Missing(reason) => {
                    return Err(VertexError::EngineMissing(reason));
                }
            }
        }

        // Multi-hop path search (BFS)
        if let Some(path) = self.find_path(from_format, target_format) {
            let mut current_input = input.to_path_buf();
            let total_steps = path.len();

            let temp_dir = std::env::temp_dir().join(format!("vertex_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&temp_dir);

            for (idx, (step_from, step_to)) in path.iter().enumerate() {
                cancel.check()?;

                let is_last = idx == total_steps - 1;
                let step_output = if is_last {
                    output_path.clone()
                } else {
                    temp_dir.join(format!("step_{}_{}.{}", idx, stem(input), step_to.extension()))
                };

                let conv = self
                    .find_converter(*step_from, *step_to)
                    .ok_or_else(|| VertexError::NoConversionRoute {
                        from: step_from.to_string(),
                        to: step_to.to_string(),
                    })?;

                let step_start = idx as f32 / total_steps as f32;
                let step_weight = 1.0 / total_steps as f32;

                conv.convert(
                    &current_input,
                    &step_output,
                    opts,
                    &|p| progress(step_start + p * step_weight),
                    cancel,
                )?;

                // Clean up previous temporary file if it was intermediate
                if idx > 0 && current_input.starts_with(&temp_dir) {
                    let _ = std::fs::remove_file(&current_input);
                }

                current_input = step_output;
            }

            let _ = std::fs::remove_dir(&temp_dir);
            return Ok(output_path);
        }

        Err(VertexError::NoConversionRoute {
            from: from_format.to_string(),
            to: target_format.to_string(),
        })
    }

    /// Breadth-First-Search for multi-hop conversion
    fn find_path(&self, from: Format, to: Format) -> Option<Vec<(Format, Format)>> {
        let mut queue: VecDeque<(Format, Vec<(Format, Format)>)> = VecDeque::new();
        let mut visited: HashSet<Format> = HashSet::new();

        queue.push_back((from, Vec::new()));
        visited.insert(from);

        while let Some((curr, path)) = queue.pop_front() {
            if curr == to {
                return Some(path);
            }

            for conv in &self.converters {
                if conv.available().is_ok() && conv.sources().contains(&curr) {
                    for next in conv.targets(curr) {
                        if !visited.contains(&next) {
                            visited.insert(next);
                            let mut next_path = path.clone();
                            next_path.push((curr, next));
                            queue.push_back((next, next_path));
                        }
                    }
                }
            }
        }

        None
    }
}

fn stem(path: &Path) -> &str {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("file")
}

fn sort_targets_by_affinity(
    targets: &mut [(Format, Availability)],
    preferred_category: crate::format::Category,
) {
    targets.sort_by(|(a, _), (b, _)| {
        let a_same = a.category() == preferred_category;
        let b_same = b.category() == preferred_category;

        match (a_same, b_same) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let p_a = format_priority(*a);
                let p_b = format_priority(*b);
                p_a.cmp(&p_b).then_with(|| a.label().cmp(b.label()))
            }
        }
    });
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
