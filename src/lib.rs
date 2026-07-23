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

pub use self::hasher::{DirectoryHasher, DirectoryHasherIter};
pub use manifest::*;
pub use verifier::*;

/// The name of the file where instances of [`Manifest`] will be serialized/deserialized to/from.
pub const HASHFILE: &str = ".b3hash";

/// An error that might occur when hashing a directory.
#[derive(Debug, thiserror::Error)]
pub enum HashingError {
    /// Indicates that file hashing was canceled early.
    #[error("file hashing canceled early")]
    Canceled,

    /// Indicates that there was an underlying IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// An error that might occur when serializing/deserializing a [`Manifest`].
#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    /// Indicates that there was an underlying IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Indicates that there was an underlying Json error.
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
    for entry in entries {
        // Explicitly use LE to avoid differences across platforms.
        let size_as_bytes = entry.size.to_le_bytes();
        // WARNING: Changing the order in which these fields are
        // fed to the hasher will change the finalized hash value,
        // so don't do that :).
        hasher.update(entry.hash.as_bytes());
        hasher.update(&size_as_bytes);
        size += entry.size;
    }
    let hash = hasher.finalize();
    (hash, size)
}
