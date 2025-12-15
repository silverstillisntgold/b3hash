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

/// The default file used when building the list of entries that will be ignored.
///
/// If the ignorefile you wish to use doesn't have this name, you'll need to specify it.
pub const IGNOREFILE: &str = ".gitignore";

/// Current version of the manifest's format.
pub const MANIFEST_VERSION: u64 = 1;
