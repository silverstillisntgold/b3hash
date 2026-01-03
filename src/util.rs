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
    Channel(#[from] crossbeam_channel::SendError<Event>),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Events which can be emitted during hashing and verifying.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Event {
    FileDiscoveryStarted,
    /// Contains the number of files discovered.
    FileDiscoveryCompleted(usize),

    FileSortingStarted,
    FileSortingCompleted,

    FileHashingStarted,
    /// Contains the path of the hashed file, relative to the provided root directory.
    FileHashed(Utf8PathBuf),
    FileHashingCompleted,

    DirectoryHashingStarted,
    DirectoryHashingCompleted,

    DirectoryVerificationStarted,
    /// Contains `true` if the verification was a success.
    DirectoryVerificationCompleted(bool),
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
