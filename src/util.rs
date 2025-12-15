use camino::Utf8PathBuf;
use crossbeam_channel::SendError;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

/// Error type for the crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("file hashing cancelled early")]
    Cancelled,

    #[error(transparent)]
    Channel(#[from] SendError<Event>),

    #[error(transparent)]
    Glob(#[from] globset::Error),

    #[error(transparent)]
    Hex(#[from] blake3::HexError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub enum Event {
    FileDiscoveryStarted,
    /// Contains the number of files discovered.
    FileDiscoveryCompleted(usize),

    FileSortingStarted,
    FileSortingCompleted,

    FileHashingStarted,
    /// Contains the path of the hashed file.
    FileHashed(Utf8PathBuf),
    FileHashingCompleted,

    DirectoryHashingStarted,
    DirectoryHashingCompleted,
}

/// Pointer into an [`AtomicBool`] used to signal that an operation
/// should be canceled early.
#[derive(Clone, Debug, Default)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    /// Cancels the associated operation and drops `self`.
    pub fn cancel(self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Loads the value of the bool.
    pub fn load(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
