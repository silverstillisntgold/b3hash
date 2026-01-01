/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

//#![warn(missing_docs)]

mod file;
mod hasher;
mod manifest;
mod util;

pub use crate::hasher::*;
pub use crate::manifest::*;
pub use crate::util::*;

/// The name of the file where instances of [`Manifest`] will be serialized/deserialized to/from.
pub const HASHFILE: &str = ".b3hash";

/// Current version of the manifest's format.
///
/// This value will always match the major version of the crate.
pub const MANIFEST_VERSION: u64 = 0;
