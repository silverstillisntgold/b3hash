/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

#![allow(unused)]

mod file;

use blake3::Hash;
use bon::Builder;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// The name of file where [`Manifest`] will be serialized to.
pub const HASHFILE: &str = ".b3hash";
/// The default file used when building the list of entries that will be ignored.
pub const IGNOREFILE: &str = ".gitignore";

/// Error type for the crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

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
    dir_size: Option<u64>,
    entries: Vec<Entry>,
}

impl Manifest {
    pub fn dir_size(&mut self) -> u64 {
        match self.dir_size {
            Some(val) => val,
            None => {
                let dir_size = self.entries.iter().map(|e| e.size).sum();
                self.dir_size = Some(dir_size);
                dir_size
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    size: u64,
    hash: Hash,
    path: Utf8PathBuf,
}

#[derive(Debug)]
pub enum Event {
    Something,
    OrAnother,
}

#[derive(Builder, Debug)]
pub struct DirectoryHasher {
    directory_path: Utf8PathBuf,

    custom_ignore_source: Option<String>,

    num_threads: Option<NonZeroUsize>,

    #[builder(default = false)]
    follow_symlinks: bool,

    #[builder(default = true)]
    respect_hidden: bool,

    #[builder(default = true)]
    respect_ignore: bool,

    progress_channel: Option<crossbeam_channel::Sender<Event>>,

    cancellation: Option<Arc<AtomicBool>>,
}

impl DirectoryHasher {
    pub fn hash(self) -> Manifest {
        todo!()
    }

    pub fn verify(self) -> ! {
        todo!()
    }
}
