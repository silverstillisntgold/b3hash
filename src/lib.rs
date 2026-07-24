/*!
# B3Hash

**This crate is considered feature complete.**

A crate for creating and validating directory hashfiles.
*/

#![forbid(missing_docs, unsafe_code)]

mod file;
mod hasher;
mod manifest;
mod verifier;

use std::sync::mpsc::SendError;

pub use self::hasher::{DirectoryHasher, DirectoryHasherIter};
pub use manifest::*;
pub use verifier::*;

pub use camino::{Utf8Path, Utf8PathBuf};

/// The name of the file where instances of [`Manifest`] will be serialized/deserialized to/from.
pub const HASHFILE: &str = ".b3hash";

/// An error that might occur when hashing a directory.
#[derive(Debug, thiserror::Error)]
pub enum HashingError {
    /// Indicates that file hashing was canceled early.
    #[error("file hashing was canceled early")]
    Canceled,

    /// Indicates that there was an IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Indicates that there was a channel send error.
    #[error(transparent)]
    Send(#[from] SendError<Utf8PathBuf>),
}

/// An error that might occur when serializing/deserializing a [`Manifest`].
#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    /// Indicates that there was an IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Indicates that there was a JSON error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Indicates that the manifest has an invalid format.
    #[error("manifest has an invalid format")]
    Validation,
}

/// Calculates the cumulative hash and size of all elements in `entries`.
fn hash_entries(entries: &[Entry]) -> (blake3::Hash, u64) {
    let mut hasher = blake3::Hasher::new();
    let mut size = 0;
    // WARNING: Changing the order in which these fields are fed to the
    // hasher will change the finalized hash value, so don't do that :).
    for entry in entries {
        // Explicitly use LE to avoid differences across platforms.
        let size_as_bytes = entry.size.to_le_bytes();
        // Hashing each component individually instead of converting the
        // path to a str and hashing that means we avoid having to worry
        // about path component separator normalization.
        for s in entry
            .path()
            .components()
            .map(|component| component.as_str())
        {
            // Explicitly use LE to avoid differences across platforms.
            let len_as_bytes = s.len().to_le_bytes();
            hasher.update(s.as_bytes());
            hasher.update(&len_as_bytes);
        }
        hasher.update(entry.hash.as_bytes());
        hasher.update(&size_as_bytes);
        size += entry.size;
    }
    let hash = hasher.finalize();
    (hash, size)
}
