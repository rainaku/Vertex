use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::cancel::CancelToken;
use crate::error::Result;
use crate::format::Format;
use crate::options::Options;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum Availability {
    Ok,
    Missing(String),
}

/// Classification already established by the registry for this conversion hop.
#[derive(Clone, Copy)]
pub struct DetectedInput<'a> {
    pub path: &'a Path,
    pub format: Format,
}

impl Availability {
    pub fn is_ok(&self) -> bool {
        matches!(self, Availability::Ok)
    }
}

pub trait Converter: Send + Sync {
    fn name(&self) -> &'static str;
    fn sources(&self) -> &[Format];
    fn targets(&self, from: Format) -> Vec<Format>;
    fn available(&self) -> Availability;
    fn convert_detected(
        &self,
        input: DetectedInput<'_>,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        self.convert(input.path, output, opts, progress, cancel)
    }
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()>;
}
