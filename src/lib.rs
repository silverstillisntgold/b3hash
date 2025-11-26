/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

#![allow(unused)]

mod arcvec;
mod file;

use blake3::Hash;
use bon::Builder;
use camino::Utf8PathBuf;
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// The name of file where [`Manifest`] will be serialized to.
pub const HASHFILE: &str = ".b3hash";

/// The default file used when building the list of entries that will be ignored.
///
/// If the ignorefile you wish to use doesn't have this name, you'll need to specify it.
pub const IGNOREFILE: &str = ".gitignore";

/// Error type for the crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Glob(#[from] globset::Error),

    #[error(transparent)]
    Hex(#[from] blake3::HexError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    version: u64,
    dir_name: String,
    dir_hash: Hash,
    dir_size: u64,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    path: Utf8PathBuf,
    hash: Hash,
    size: u64,
}

#[derive(Debug)]
pub enum Event {
    FileDiscoveryStarted,
    FileDiscoveryCompleted(u64),
    FileHashingStarted,
    FileHashingCompleted,
    FileHashingFailed,
    FileHashed(Utf8PathBuf),
}

/// The only required field is `directory_path`.
#[derive(Builder, Debug)]
pub struct DirectoryHasher {
    /// Specifies the directory which will be hashed.
    directory_path: Utf8PathBuf,

    /// Provides a specific file or file path of an ignorefile.
    custom_ignore_source: Option<Utf8PathBuf>,

    /// Specifies the amount of threads the underlaying rayon threadpool should use.
    ///
    /// Useful if you'll be running [`DirectoryHasher`] in a background thread
    /// and don't want it consuming all CPU resources, as is the default.
    num_threads: Option<NonZeroUsize>,

    #[builder(default = true)]
    respect_hidden: bool,

    #[builder(default = true)]
    respect_ignore: bool,

    /// Provides a channel which will be used to send internal information to the
    /// receiving end as operation proceeds.
    progress_channel: Option<Sender<Event>>,

    /// A flag for signaling early cancellation from outside.
    cancellation: Option<Arc<AtomicBool>>,
}

impl DirectoryHasher {
    pub fn hash(self) -> Result<Manifest, Error> {
        let dir_path_str = self.directory_path.as_str();
        // Ensure that we never include a leading `/` or `\` when stripping paths.
        let prefix_len = if dir_path_str.ends_with('/') || dir_path_str.ends_with('\\') {
            dir_path_str.len()
        } else {
            dir_path_str.len() + 1
        };

        let file_list = {
            let mut tmp = file::FileFinder::from(&self).find()?;
            tmp.sort_unstable_by(|a, b| {
                // We don't know how long the given prefix will be, so it's best
                // to strip it out to minimize the time spent sorting.
                //
                // SAFETY: Since all files are descendants of dir_path,
                // they all have dir_path as a prefix.
                let a_stripped = unsafe { a.as_str().get_unchecked(prefix_len..) };
                let b_stripped = unsafe { b.as_str().get_unchecked(prefix_len..) };
                a_stripped.cmp(b_stripped)
            });
            tmp
        };

        todo!()
    }

    pub fn verify(self) -> ! {
        todo!()
    }
}
