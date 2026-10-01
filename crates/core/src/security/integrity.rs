use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const OFFICIAL_REPO_OWNER: &str = "rainaku";
pub const OFFICIAL_REPO_NAME: &str = "Vertex";
pub const OFFICIAL_RELEASES_URL: &str = "https://github.com/rainaku/Vertex/releases";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IntegrityCheckStatus {
    Verified,
    HashMismatch,
    ReleaseNotFound,
    NetworkError,
    Skipped,
}

pub struct AppIntegrityService;

impl AppIntegrityService {
    /// Computes the SHA-256 hash of a file as a lowercase hex string.
    pub fn compute_file_sha256(path: &Path) -> std::io::Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Checks whether the running binary was downloaded from a trusted official domain
    /// by reading the Windows NTFS Zone.Identifier alternate data stream.
    /// Returns (is_trusted, Option<untrusted_url>).
    pub fn check_download_origin(file_path: &Path) -> (bool, Option<String>) {
        if !file_path.exists() {
            return (true, None);
        }

        #[cfg(target_os = "windows")]
        {
            let zone_stream_path = format!("{}:Zone.Identifier", file_path.display());
            let Ok(content) = std::fs::read_to_string(&zone_stream_path) else {
                // No Mark of the Web stream present (e.g. local build, unblocked, or non-NTFS).
                return (true, None);
            };

            let (host_url, referrer_url) = Self::parse_zone_identifier(&content);

            if let Some(host) = host_url {
                if !host.is_empty() && !Self::is_trusted_download_domain(&host) {
                    return (false, Some(host));
                }
            }

            if let Some(referrer) = referrer_url {
                if !referrer.is_empty() && !Self::is_trusted_download_domain(&referrer) {
                    return (false, Some(referrer));
                }
            }
        }

        (true, None)
    }

    /// Parses Zone.Identifier stream content to extract HostUrl and ReferrerUrl.
    pub fn parse_zone_identifier(content: &str) -> (Option<String>, Option<String>) {
        let mut host_url = None;
        let mut referrer_url = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("HostUrl=") {
                host_url = Some(val.trim().to_string());
            } else if let Some(val) = trimmed.strip_prefix("ReferrerUrl=") {
                referrer_url = Some(val.trim().to_string());
            }
        }

        (host_url, referrer_url)
    }

    /// Validates whether a URL belongs to official GitHub releases or trusted CDNs.
    pub fn is_trusted_download_domain(url: &str) -> bool {
        if url.chars().any(char::is_control) {
            return false;
        }
        let Ok(parsed) = url::Url::parse(url.trim()) else {
            return false;
        };
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port_or_known_default() != Some(443)
        {
            return false;
        }
        let host = parsed.host_str().unwrap_or_default();
        match host {
            "github.com" | "raw.githubusercontent.com" => {
                let mut segments = parsed.path_segments().into_iter().flatten();
                segments
                    .next()
                    .is_some_and(|v| v.eq_ignore_ascii_case(OFFICIAL_REPO_OWNER))
                    && segments
                        .next()
                        .is_some_and(|v| v.eq_ignore_ascii_case(OFFICIAL_REPO_NAME))
            }
            // Shared GitHub CDNs are origin hints, not proof of publisher identity.
            "objects.githubusercontent.com" | "release-assets.githubusercontent.com" => true,
            _ => false,
        }
    }
    /// Checks whether the running environment is development or test, in which case online checks are skipped.
    pub fn is_dev_or_test_environment() -> bool {
        cfg!(debug_assertions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_trusted_download_domains() {
        assert!(AppIntegrityService::is_trusted_download_domain(
            "https://github.com/rainaku/Vertex/releases/download/v0.1.0/Vertex_0.1.0_x64-setup.exe"
        ));
        assert!(AppIntegrityService::is_trusted_download_domain(
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/..."
        ));
        assert!(!AppIntegrityService::is_trusted_download_domain(
            "http://localhost:5173"
        ));
        assert!(!AppIntegrityService::is_trusted_download_domain(
            "about:internet"
        ));

        // Untrusted 3rd party mirrors
        assert!(!AppIntegrityService::is_trusted_download_domain(
            "https://malicious-downloads-mirror.com/vertex-repack.exe"
        ));
        assert!(!AppIntegrityService::is_trusted_download_domain(
            "https://fake-software-portal.ru/files/Vertex.exe"
        ));
    }

    #[test]
    fn rejects_spoofed_origins() {
        for url in [
            "https://evil.example/?next=github.com/rainaku/Vertex",
            "https://objects.githubusercontent.com.evil.example/file",
            "https://github.com@evil.example/rainaku/Vertex",
            "https://github.com/rainaku/Vertex-fake/releases",
            "https://github.com/other/Vertex/releases",
            "https://evil.example/localhost",
            "http://github.com/rainaku/Vertex/releases",
        ] {
            assert!(
                !AppIntegrityService::is_trusted_download_domain(url),
                "{url}"
            );
        }
    }

    #[test]
    fn test_parse_zone_identifier() {
        let content = "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://github.com/rainaku/Vertex/releases\r\nHostUrl=https://objects.githubusercontent.com/...\r\n";
        let (host, referrer) = AppIntegrityService::parse_zone_identifier(content);
        assert_eq!(
            host,
            Some("https://objects.githubusercontent.com/...".to_string())
        );
        assert_eq!(
            referrer,
            Some("https://github.com/rainaku/Vertex/releases".to_string())
        );
    }

    #[test]
    fn test_compute_file_sha256() {
        let mut temp = tempfile::NamedTempFile::new().unwrap();
        temp.write_all(b"Vertex Secure Hash Test Content").unwrap();
        let hash = AppIntegrityService::compute_file_sha256(temp.path()).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
