/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

//#![warn(missing_docs)]

mod file;
mod hasher;
mod manifest;
mod verifier;

pub use hasher::*;
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
    Channel(#[from] crossbeam_channel::SendError<camino::Utf8PathBuf>),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// An error which can occur when serializing or deserializing [`Manifest`](crate::manifest::Manifest).
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
