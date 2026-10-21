use std::path::{Path, PathBuf};

/// Safe URI and external process launcher guards.
pub struct SafeLauncher;

impl SafeLauncher {
    /// Validates whether a URL has a safe protocol for opening in an external browser.
    /// Only allows `https://`, `http://`, and `mailto:`.
    pub fn is_safe_url(raw: &str) -> bool {
        if raw.chars().any(char::is_control) {
            return false;
        }
        let Ok(url) = url::Url::parse(raw.trim()) else {
            return false;
        };
        match url.scheme() {
            "http" | "https" => {
                url.host_str().is_some() && url.username().is_empty() && url.password().is_none()
            }
            "mailto" => !url.path().is_empty(),
            _ => false,
        }
    }

    pub fn sanitize_explorer_path(raw_path: &Path) -> Result<PathBuf, String> {
        if raw_path
            .as_os_str()
            .to_string_lossy()
            .chars()
            .any(|c| c.is_control() || ['"', '|', '<', '>'].contains(&c))
        {
            return Err("Path contains illegal characters".into());
        }
        #[cfg(windows)]
        validate_local_path(raw_path, 0)?;
        #[cfg(not(windows))]
        {
            let bytes = raw_path.as_os_str().as_encoded_bytes();
            if bytes.starts_with(b"\\\\") || bytes.starts_with(b"//") {
                return Err("Network paths are not permitted".into());
            }
        }
        let canonical = raw_path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {e}"))?;
        #[cfg(windows)]
        if !is_local_prefix(&canonical) {
            return Err("Network targets are not permitted".into());
        }
        Ok(crate::path_utils::external_path(&canonical))
    }
}

#[cfg(windows)]
fn is_local_prefix(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) => true,
            // Local mounted volumes are legal, including folder mount points.
            Prefix::Verbatim(name) => name.as_encoded_bytes().starts_with(b"Volume{"),
            _ => false,
        },
        _ => {
            let bytes = path.as_os_str().as_encoded_bytes();
            !(bytes.starts_with(b"\\\\") || bytes.starts_with(b"//"))
        }
    }
}

#[cfg(windows)]
fn validate_local_path(path: &Path, depth: u8) -> Result<(), String> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileAttributeTagInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_TAG_INFO,
    };
    if depth > 32 || !is_local_prefix(path) {
        return Err("Network paths or cyclic links are not permitted".into());
    }
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    let ancestors: Vec<_> = absolute.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if metadata.file_attributes() & 0x400 == 0 {
            continue;
        }
        // Query the tag without following it. Cloud/dedup tags do not redirect
        // names; reject only a name-surrogate that resolves to a network target.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .access_mode(0)
            .custom_flags(0x00200000 | 0x02000000)
            .open(ancestor)
            .map_err(|e| e.to_string())?;
        let mut info: FILE_ATTRIBUTE_TAG_INFO = unsafe { std::mem::zeroed() };
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileAttributeTagInfo,
                (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if info.ReparseTag & 0x20000000 != 0 {
            let target = std::fs::read_link(ancestor).map_err(|e| e.to_string())?;
            let resolved = if target.is_absolute() {
                target
            } else {
                crate::path_utils::parent_dir(ancestor).join(target)
            };
            validate_local_path(&resolved, depth + 1)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permits_safe_schemes() {
        assert!(SafeLauncher::is_safe_url(
            "https://github.com/rainaku/Vertex"
        ));
        assert!(SafeLauncher::is_safe_url("http://localhost:5173"));
        assert!(SafeLauncher::is_safe_url("mailto:support@vertex.app"));
    }

    #[test]
    fn test_blocks_dangerous_schemes() {
        assert!(!SafeLauncher::is_safe_url(
            "file:///C:/Windows/System32/cmd.exe"
        ));
        assert!(!SafeLauncher::is_safe_url("javascript:alert(1)"));
        assert!(!SafeLauncher::is_safe_url(
            "data:text/html,<script>alert(1)</script>"
        ));
        assert!(!SafeLauncher::is_safe_url("cmd.exe /c calc.exe"));
        assert!(!SafeLauncher::is_safe_url("powershell.exe -enc AAA"));
        assert!(!SafeLauncher::is_safe_url("ms-settings:network"));
        assert!(!SafeLauncher::is_safe_url("   "));
        assert!(!SafeLauncher::is_safe_url(""));
    }

    #[test]
    fn test_rejects_unc_explorer_path() {
        let unc_path = Path::new(r"\\attacker-server\share\payload.exe");
        assert!(SafeLauncher::sanitize_explorer_path(unc_path).is_err());
    }

    #[test]
    fn permits_shell_metacharacters_in_real_filenames() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Design & Assets; final.png");
        std::fs::write(&path, b"image").unwrap();
        assert!(SafeLauncher::sanitize_explorer_path(&path).is_ok());
    }

    #[test]
    fn rejects_forward_slash_network_paths() {
        assert!(
            SafeLauncher::sanitize_explorer_path(Path::new("//server/share/file.png")).is_err()
        );
    }
}
