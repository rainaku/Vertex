use std::path::{Path, PathBuf};

use crate::error::{Result, VertexError};
use crate::format::Format;
use crate::options::{CollisionPolicy, Options};

/// Resolve safe target file path according to Options and CollisionPolicy.
pub fn resolve_target_path(
    input: &Path,
    target_format: Format,
    opts: &Options,
) -> Result<PathBuf> {
    let output_dir = if let Some(dir) = &opts.output_dir {
        dir.clone()
    } else {
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| VertexError::Io {
            path: output_dir.clone(),
            source: e,
        })?;
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = target_format.extension();

    let candidate = output_dir.join(format!("{}.{}", stem, ext));

    let is_same_as_input = if let (Ok(c), Ok(i)) = (candidate.canonicalize(), input.canonicalize()) {
        c == i
    } else {
        candidate == input
    };

    if !candidate.exists() && !is_same_as_input {
        return Ok(candidate);
    }

    if is_same_as_input {
        // If converting to the same format in the same directory,
        // we must never overwrite the input file while reading it.
        let mut counter = 1;
        loop {
            let candidate = output_dir.join(format!("{} ({}).{}", stem, counter, ext));
            if !candidate.exists() {
                return Ok(candidate);
            }
            counter += 1;
        }
    }

    match opts.collision_policy {
        CollisionPolicy::Overwrite => Ok(candidate),
        CollisionPolicy::FailIfExists => Err(VertexError::Io {
            path: candidate,
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Target file already exists and overwrite policy is disabled",
            ),
        }),
        CollisionPolicy::RenameWithSuffix => {
            let mut counter = 1;
            loop {
                let candidate = output_dir.join(format!("{} ({}).{}", stem, counter, ext));
                if !candidate.exists() {
                    return Ok(candidate);
                }
                counter += 1;
            }
        }
    }
}
