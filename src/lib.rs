/*!
# B3Hash

**This crate is considered feature complete.**

A crate for creating and validating directory hashfiles.
*/

#![deny(missing_docs)]

mod file;
mod hasher;
mod manifest;
mod verifier;

pub use self::hasher::{DirectoryHasher, DirectoryHasherIter};
pub use manifest::*;
pub use verifier::*;

/// The name of the file where instances of [`Manifest`] will be serialized/deserialized to/from.
pub const HASHFILE: &str = ".b3hash";

/// An error which can occur when hashing a directory.
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum HashingError {
    #[error("file hashing canceled early")]
    Canceled,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// An error which can occur when serializing/deserializing a [`Manifest`].
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Determines the cumulative hash of all members of `entries`.
fn hash_entries(entries: &[Entry]) -> (blake3::Hash, u64) {
    let mut hasher = blake3::Hasher::new();
    let mut size = 0;
    for entry in entries {
        entry.hash_fields(&mut hasher);
        size += entry.size;
    }
    let hash = hasher.finalize();
    (hash, size)
}
