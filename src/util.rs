use camino::Utf8PathBuf;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Error type for the crate.
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file hashing cancelled early")]
    Cancelled,

    #[error(transparent)]
    Channel(#[from] crossbeam_channel::SendError<Utf8PathBuf>),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Pointer into an [`AtomicBool`] used to signal that an operation should be canceled early.
#[derive(Clone, Debug, Default)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    /// Cancels the associated operation and drops `self`.
    pub fn cancel(self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Loads the value of the bool.
    pub(crate) fn load(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
