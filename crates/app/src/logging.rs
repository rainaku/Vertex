use std::io::{self, Write};
use vertex_core::security::SensitiveDataScrubber;

// tracing formats an event in multiple writes. Redact only after the complete
// event has been assembled, so secrets split across writes cannot bypass it.
#[derive(Default)]
pub struct RedactedWriter(Vec<u8>);

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
        let text = String::from_utf8_lossy(&self.0);
        let redacted = SensitiveDataScrubber::scrub(&text);
        let _ = io::stderr().lock().write_all(redacted.as_bytes());
    }
}
