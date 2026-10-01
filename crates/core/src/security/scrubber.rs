use regex::Regex;
use std::sync::LazyLock;

static SP_DC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(sp_dc=)[^\s;,\r\n\x22]+").unwrap());

static GOOGLE_API_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"AIza[0-9A-Za-z\-_]{16,40}").unwrap());

static GITHUB_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:ghp|gho|ghu|ghs|ghr)_[0-9A-Za-z]{36}|github_pat_[0-9A-Za-z_]{82}").unwrap()
});

static DPAPI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"enc:[A-Za-z0-9+/=]{16,}").unwrap());

static URL_SECRET_PARAM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)([?&](?:key|apikey|api_key|token|access_token|secret|client_secret)=)[^&\s\x22\x27]+",
    )
    .unwrap()
});

static BEARER_TOKEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(Bearer\s+)[A-Za-z0-9\-._~+/]+=*").unwrap());

static PASSWORD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)((?:password|passwd|pwd)\s*[:=]\s*)[^\s,;\x22\x27]+").unwrap()
});

static QUOTED_SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)((?:["']?)(?:password|passwd|pwd|token|access_token|api_key|apikey|secret|client_secret)["']?\s*[:=]\s*)(?:"[^"\r\n]*"|'[^'\r\n]*')"#).unwrap()
});

static URL_USERINFO_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(https?://)[^/\s@]+@").unwrap());

/// Redacts sensitive credentials, tokens, keys, and passwords from logs and diagnostic strings.
pub struct SensitiveDataScrubber;

impl SensitiveDataScrubber {
    pub fn scrub(input: &str) -> String {
        if input.is_empty() {
            return String::new();
        }

        let quoted = QUOTED_SECRET_REGEX.replace_all(input, "${1}[REDACTED]");
        let userinfo = URL_USERINFO_REGEX.replace_all(&quoted, "${1}[REDACTED]@");
        let s1 = SP_DC_REGEX.replace_all(&userinfo, "${1}[REDACTED]");
        let s2 = GOOGLE_API_KEY_REGEX.replace_all(&s1, "AIza[REDACTED]");
        let s3 = GITHUB_TOKEN_REGEX.replace_all(&s2, "[REDACTED_GITHUB_TOKEN]");
        let s4 = DPAPI_REGEX.replace_all(&s3, "enc:[REDACTED]");
        let s5 = URL_SECRET_PARAM_REGEX.replace_all(&s4, "${1}[REDACTED]");
        let s6 = BEARER_TOKEN_REGEX.replace_all(&s5, "${1}[REDACTED]");
        let s7 = PASSWORD_REGEX.replace_all(&s6, "${1}[REDACTED]");

        s7.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrubs_google_api_key() {
        let input = "Calling service with key AIzaSyD_abc1234567890XYZ_abcdef12345678 in request";
        let scrubbed = SensitiveDataScrubber::scrub(input);
        assert!(!scrubbed.contains("AIzaSyD_abc1234567890XYZ_abcdef12345678"));
        assert!(scrubbed.contains("AIza[REDACTED]"));
    }

    #[test]
    fn test_scrubs_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.t-ID";
        let scrubbed = SensitiveDataScrubber::scrub(input);
        assert!(!scrubbed.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(scrubbed.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn test_scrubs_url_secret_params() {
        let input = "https://api.example.com/v1?api_key=secret_123&client_secret=xyz_456";
        let scrubbed = SensitiveDataScrubber::scrub(input);
        assert!(!scrubbed.contains("secret_123"));
        assert!(!scrubbed.contains("xyz_456"));
        assert!(scrubbed.contains("api_key=[REDACTED]"));
        assert!(scrubbed.contains("client_secret=[REDACTED]"));
    }

    #[test]
    fn test_scrubs_password() {
        let input = "Config has password = superSecret1234; user=admin";
        let scrubbed = SensitiveDataScrubber::scrub(input);
        assert!(!scrubbed.contains("superSecret1234"));
        assert!(scrubbed.contains("password = [REDACTED]"));
    }

    #[test]
    fn test_scrubs_github_token() {
        let input = "git push with token ghp_123456789012345678901234567890123456";
        let scrubbed = SensitiveDataScrubber::scrub(input);
        assert!(!scrubbed.contains("ghp_123456789012345678901234567890123456"));
        assert!(scrubbed.contains("[REDACTED_GITHUB_TOKEN]"));
    }
}
