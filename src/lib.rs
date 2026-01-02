/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

//#![warn(missing_docs)]

mod file;
mod hasher;
mod manifest;
mod util;
mod verifier;

pub use crate::hasher::*;
pub use crate::manifest::*;
pub use crate::util::{CancelHandle, Error, Event};
pub use crate::verifier::*;

/// The name of the file where instances of [`Manifest`] will be serialized/deserialized to/from.
pub const HASHFILE: &str = ".b3hash";
