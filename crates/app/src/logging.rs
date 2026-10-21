use std::io::{self, Write};
use vertex_core::security::SensitiveDataScrubber;

// tracing formats an event in multiple writes. Redact only after the complete
// event has been assembled, so secrets split across writes cannot bypass it.
pub struct RedactedWriter(Vec<u8>);

impl Default for RedactedWriter {
    fn default() -> Self {
        Self(Vec::with_capacity(256))
    }
}

impl Write for RedactedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for RedactedWriter {
    fn drop(&mut self) {
        if self.0.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.0);
        let redacted = SensitiveDataScrubber::scrub(&text);
        let _ = io::stderr().lock().write_all(redacted.as_bytes());
    }
}
