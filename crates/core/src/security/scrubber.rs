use regex::{Captures, Regex};
use std::borrow::Cow;
use std::sync::LazyLock;

static SECRETS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r#"(?ix)
    (?P<assignment>\b["']?(?:password|passwd|pwd|token|access_token|api_key|apikey|key|secret|client_secret|sp_dc)["']?\s*[:=]\s*)
    (?:"(?:\\.|[^"\\\r\n])*"|'(?:\\.|[^'\\\r\n])*'|[^\s,;&"'<>]+)
    | (?P<userinfo>https?://)[^/\s@]+@
    | (?P<bearer>Bearer\s+)[A-Za-z0-9\-._~+/]+=*
    | (?P<google>AIza)[0-9A-Za-z\-_]{16,40}
    | (?P<github>(?:ghp|gho|ghu|ghs|ghr)_[0-9A-Za-z]{36}|github_pat_[0-9A-Za-z_]{82})
    | (?P<dpapi>enc:)[A-Za-z0-9+/=]{16,}
    "#).expect("valid credential redaction regex")
});

pub struct SensitiveDataScrubber;
impl SensitiveDataScrubber {
    /// One pass, and no allocation at all for ordinary log lines.
    pub fn scrub(input: &str) -> Cow<'_, str> {
        SECRETS.replace_all(input, |caps: &Captures<'_>| {
            if let Some(prefix) = caps.name("assignment").or_else(|| caps.name("bearer")) {
                format!("{}[REDACTED]", prefix.as_str())
            } else if let Some(prefix) = caps.name("userinfo") {
                format!("{}[REDACTED]@", prefix.as_str())
            } else if caps.name("google").is_some() {
                "AIza[REDACTED]".into()
            } else if caps.name("github").is_some() {
                "[REDACTED_GITHUB_TOKEN]".into()
            } else {
                "enc:[REDACTED]".into()
            }
        })
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
