use camino::Utf8PathBuf;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// An error which can occur when hashing a directory.
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum HashingError {
    #[error("file hashing canceled early")]
    Canceled,

    #[error(transparent)]
    Channel(#[from] crossbeam_channel::SendError<Utf8PathBuf>),

    #[error(transparent)]
    Io(#[from] io::Error),
}

/// An error which can occur when serializing or deserializing [`Manifest`](crate::manifest::Manifest).
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Pointer to an [`AtomicBool`], used to signal that an operation should be canceled early.
#[derive(Clone, Default)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    /// Cancels the associated operation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Loads the value from the bool.
    pub fn load(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
