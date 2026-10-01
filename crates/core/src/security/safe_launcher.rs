use std::path::{Path, PathBuf};

/// Safe URI and external process launcher guards.
pub struct SafeLauncher;

impl SafeLauncher {
    /// Validates whether a URL has a safe protocol for opening in an external browser.
    /// Only allows `https://`, `http://`, and `mailto:`.
    pub fn is_safe_url(url: &str) -> bool {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Explicitly reject dangerous pseudo-protocols and system commands
        let lower = trimmed.to_lowercase();
        if lower.starts_with("javascript:")
            || lower.starts_with("data:")
            || lower.starts_with("file:")
            || lower.starts_with("vbscript:")
            || lower.starts_with("cmd:")
            || lower.starts_with("powershell:")
            || lower.starts_with("shell:")
            || lower.starts_with("ms-settings:")
            || lower.starts_with("ms-appinstaller:")
        {
            return false;
        }

        // Scheme validation
        if lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:")
        {
            // Ensure no invalid control characters or embedded newlines
            if trimmed
                .chars()
                .any(|c| c.is_control() || c == '\n' || c == '\r')
            {
                return false;
            }
            return true;
        }

        false
    }

    /// Sanitizes and validates a filesystem path before revealing it in File Explorer.
    /// Rejects direct network/device paths. Arguments are passed without a shell;
    /// ordinary filename characters such as ampersands and semicolons are safe.
    pub fn sanitize_explorer_path(raw_path: &Path) -> Result<PathBuf, String> {
        let path_str = raw_path.to_string_lossy();

        // Reject control characters and invalid Windows filename characters.
        if path_str.chars().any(|c| {
            c.is_control() || c == '\n' || c == '\r' || c == '"' || c == '|' || c == '<' || c == '>'
        }) {
            return Err("Path contains illegal characters or command injection tokens".to_string());
        }

        // Reject UNC paths that start with \\ (prevent SMB credential leakage / remote execution)
        if path_str.replace('/', "\\").starts_with(r"\\") {
            return Err("UNC network paths are not permitted for security reasons".to_string());
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // Inspect ancestors from the root before canonicalization can follow
            // a junction/symlink to a remote share. This is a launch guard, not
            // a sandbox against concurrent filesystem modifications.
            let absolute = std::path::absolute(raw_path).map_err(|e| e.to_string())?;
            let mut ancestors: Vec<_> = absolute.ancestors().collect();
            ancestors.reverse();
            for ancestor in ancestors {
                let metadata = std::fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("Reparse-point paths are not permitted for Explorer launch".into());
                }
            }
        }

        // Return canonicalized or verified path
        let canonical = raw_path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

        // Strip \\?\ prefix on Windows if present for explorer compatibility
        let canonical_str = canonical.to_string_lossy();
        if canonical_str.starts_with(r"\\?\UNC\")
            || (canonical_str.starts_with(r"\\") && !canonical_str.starts_with(r"\\?\"))
        {
            return Err("Network targets are not permitted".to_string());
        }
        let cleaned = if let Some(stripped) = canonical_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            canonical
        };

        Ok(cleaned)
    }
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
