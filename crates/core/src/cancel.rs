use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{Result, VertexError};

#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(VertexError::Cancelled)
        } else {
            Ok(())
        }
    }
}
