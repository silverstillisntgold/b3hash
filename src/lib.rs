/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

#![allow(unused)]
//#![deny(missing_docs)]

mod arcvec;
mod file;

use blake3::{Hash, Hasher};
use bon::Builder;
use camino::Utf8PathBuf;
use crossbeam_channel::Sender;
use file::FileFinder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CURRENT_VERSION: u64 = 1;

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

    #[error("file hashing cancelled early")]
    Cancelled,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    version: u64,
    directory_name: String,
    directory_hash: Hash,
    directory_size: u64,
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
    FileDiscoveryCompleted(usize),
    FileHashingStarted,
    FileHashed(Utf8PathBuf),
    FileHashingCompleted,
    DirectoryHashingStarted,
    DirectoryHashingCompleted,
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
    should_cancel: Option<AtomicBool>,
}

impl DirectoryHasher {
    pub fn hash(self) -> Result<Manifest, Error> {
        // Ensure that we never include a leading `/` or `\` when stripping paths.
        let prefix_len = if self.directory_path.as_str().ends_with('/')
            || self.directory_path.as_str().ends_with('\\')
        {
            self.directory_path.as_str().len()
        } else {
            self.directory_path.as_str().len() + 1
        };

        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileDiscoveryStarted);
        }
        let file_list = self.get_file_list(prefix_len)?;
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileDiscoveryCompleted(file_list.len()));
        }

        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileHashingStarted);
        }
        let entries = self.hash_files(file_list, prefix_len)?;
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileHashingCompleted);
        }

        if let Some(sender) = &self.progress_channel {
            sender.send(Event::DirectoryHashingStarted);
        }
        let manifest = self.hash_directory(entries);
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::DirectoryHashingCompleted);
        }
        Ok(manifest)
    }

    fn get_file_list(&self, prefix_len: usize) -> Result<Vec<Utf8PathBuf>, Error> {
        let mut file_list = FileFinder::from(self).find()?;
        file_list.sort_unstable_by(|a, b| {
            // We don't know how long the given prefix will be, so it's best
            // to strip it out to minimize the time spent sorting.
            //
            // SAFETY: Since all files are descendants of dir_path,
            // they all must have dir_path as a prefix.
            let a_stripped = unsafe { a.as_str().get_unchecked(prefix_len..) };
            let b_stripped = unsafe { b.as_str().get_unchecked(prefix_len..) };
            a_stripped.cmp(b_stripped)
        });
        Ok(file_list)
    }

    fn hash_files(
        &self,
        file_list: Vec<Utf8PathBuf>,
        prefix_len: usize,
    ) -> Result<Vec<Entry>, Error> {
        file_list
            .into_par_iter()
            .map_with(self, |s, file| {
                if let Some(b) = &s.should_cancel
                    && b.load(Ordering::Relaxed)
                {
                    return Err(Error::Cancelled);
                }
                let mut hasher = Hasher::new();
                let reader = File::open(file.as_std_path())?;
                // Calls to `Hasher::update_reader` internally buffer 64KiB of
                // data, so we don't need to worry about doing that manually.
                hasher.update_reader(reader)?;
                // SAFETY: Since all files are descendants of dir_path,
                // they all must have dir_path as a prefix.
                let stripped_file_path = unsafe { file.as_str().get_unchecked(prefix_len..) };
                let path = oi_vei(stripped_file_path).into();
                let hash = hasher.finalize();
                let size = hasher.count();
                debug_assert!(size > 0);
                if let Some(sender) = &s.progress_channel {
                    sender.send(Event::FileHashed(file));
                }
                Ok(Entry { path, hash, size })
            })
            .collect()
    }

    fn hash_directory(&self, entries: Vec<Entry>) -> Manifest {
        let directory_name = self
            .directory_path
            .file_name()
            .unwrap_or(self.directory_path.as_str())
            .to_string();
        let mut hasher = Hasher::new();
        let mut directory_size = 0;
        for entry in &entries {
            hasher.update(entry.hash.as_bytes());
            directory_size += entry.size;
        }
        let directory_hash = hasher.finalize();
        Manifest {
            version: CURRENT_VERSION,
            directory_name,
            directory_hash,
            directory_size,
            entries,
        }
    }

    pub fn verify(self) {
        todo!();
    }
}

/// Windows always has to be so funny and unique >:(
#[inline]
fn oi_vei(s: &str) -> String {
    if cfg!(windows) {
        // Codegen for this shit is actually insanely good. Also fuck windows.
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
}
