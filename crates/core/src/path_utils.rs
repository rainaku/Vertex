use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::{CollisionPolicy, Options};
use std::path::{Path, PathBuf};

pub fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Normalize file URLs at the IPC boundary, with percent decoding by a URL parser.
pub fn ipc_path(raw: &str) -> std::result::Result<PathBuf, String> {
    if raw
        .get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("file:"))
    {
        url::Url::parse(raw)
            .map_err(|e| e.to_string())?
            .to_file_path()
            .map_err(|_| "Invalid file URL".into())
    } else if raw.is_empty() || raw.contains('\0') {
        Err("Invalid empty path or NUL character".into())
    } else {
        Ok(PathBuf::from(raw))
    }
}

/// Includes symlinks and hard links when both paths exist.
pub fn same_file(a: &Path, b: &Path) -> bool {
    same_file::is_same_file(a, b).unwrap_or(false)
}

pub fn ensure_distinct(input: &Path, output: &Path) -> Result<()> {
    if same_file(input, output) {
        return Err(VertexError::InvalidOptions(
            "Input and output must be different files".into(),
        ));
    }
    Ok(())
}

fn safe_stem(input: &Path) -> String {
    let name = input
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let stem =
        if name.len() > 7 && name.as_bytes()[name.len() - 7..].eq_ignore_ascii_case(b".tar.gz") {
            &name[..name.len() - 7]
        } else {
            input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output")
        };
    let mut stem: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    stem.truncate(stem.trim_end_matches(['.', ' ']).len());
    if stem.is_empty() {
        stem.push_str("output");
    }
    let base = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|p| {
        base.strip_prefix(p).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if reserved {
        stem.insert(0, '_');
    }
    stem
}

/// This chooses a name, not a reservation. Publishing must still use an atomic
/// no-clobber operation and retry on AlreadyExists for RenameWithSuffix.
pub fn resolve_target_path(input: &Path, format: Format, opts: &Options) -> Result<PathBuf> {
    let opts = opts.normalized();
    let directory = opts
        .output_dir
        .as_deref()
        .unwrap_or_else(|| parent_dir(input));
    std::fs::create_dir_all(directory).map_err(|source| VertexError::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let stem = safe_stem(input);
    let candidate = directory.join(format!("{stem}.{}", format.extension()));
    if !candidate.try_exists().map_err(|source| VertexError::Io {
        path: candidate.clone(),
        source,
    })? {
        return Ok(candidate);
    }
    let is_input = same_file(input, &candidate);
    if !is_input {
        match opts.collision_policy {
            CollisionPolicy::Overwrite => return Ok(candidate),
            CollisionPolicy::FailIfExists => {
                return Err(VertexError::Io {
                    path: candidate,
                    source: std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "Target already exists",
                    ),
                })
            }
            CollisionPolicy::RenameWithSuffix => {}
        }
    }
    for counter in 1u64.. {
        let next = directory.join(format!("{stem} ({counter}).{}", format.extension()));
        if !next.try_exists().map_err(|source| VertexError::Io {
            path: next.clone(),
            source,
        })? {
            return Ok(next);
        }
    }
    unreachable!()
}

/// Temporary output owns cleanup on every error, cancellation and unwind.
pub fn stage_output(output: &Path) -> Result<tempfile::TempPath> {
    tempfile::Builder::new()
        .prefix(".vertex-")
        .suffix(&format!(
            ".{}",
            output.extension().and_then(|s| s.to_str()).unwrap_or("tmp")
        ))
        .tempfile_in(parent_dir(output))
        .map(|file| file.into_temp_path())
        .map_err(|source| VertexError::Io {
            path: output.to_path_buf(),
            source,
        })
}

pub fn publish_exact(staged: tempfile::TempPath, output: &Path) -> Result<()> {
    staged.persist(output).map_err(|e| VertexError::Io {
        path: output.to_path_buf(),
        source: e.error,
    })
}

/// Keep native non-UTF8 paths intact; only strip local extended-length prefixes
/// for APIs (FFmpeg/Explorer) that misinterpret them as URL schemes.
pub fn external_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if let Some(Component::Prefix(prefix)) = path.components().next() {
            if let Prefix::VerbatimDisk(letter) = prefix.kind() {
                let mut normal = PathBuf::from(format!("{}:\\", char::from(letter)));
                for c in path.components().skip(1) {
                    if !matches!(c, Component::RootDir) {
                        normal.push(c.as_os_str());
                    }
                }
                return normal;
            }
        }
    }
    path.to_path_buf()
}
