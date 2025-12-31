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

/// The name of file where [`Manifest`] will be serialized to.
pub const HASHFILE: &str = ".b3hash";

/// Current version of the manifest's format.
pub const MANIFEST_VERSION: u64 = 1;
