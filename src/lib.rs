/*!
# B3Hash

A crate for creating and validating directory hashfiles.
*/

//#![deny(missing_docs)]

mod file;

use blake3::{Hash, Hasher};
use bon::Builder;
use camino::Utf8PathBuf;
use crossbeam_channel::{SendError, Sender};
use file::FileFinder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, io};

/// The name of file where [`Manifest`] will be serialized to.
pub const HASHFILE: &str = ".b3hash";

/// The default file used when building the list of entries that will be ignored.
///
/// If the ignorefile you wish to use doesn't have this name, you'll need to specify it.
pub const IGNOREFILE: &str = ".gitignore";

/// Current version of the manifest's format.
pub const MANIFEST_VERSION: u64 = 1;

/// Error type for the crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file hashing cancelled early")]
    Cancelled,

    #[error(transparent)]
    Channel(#[from] SendError<Event>),

    #[error(transparent)]
    Glob(#[from] globset::Error),

    #[error(transparent)]
    Hex(#[from] blake3::HexError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    version: u64,
    directory_name: String,
    directory_hash: Hash,
    directory_size: u64,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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

    #[builder(default = true)]
    respect_hidden: bool,

    #[builder(default = true)]
    respect_ignore: bool,

    #[builder(default = false)]
    allow_missing_ignore: bool,

    progress_channel: Option<Sender<Event>>,

    cancel_flag: Option<AtomicBool>,
}

impl DirectoryHasher {
    #[inline(never)]
    pub fn hash(self) -> Result<Manifest, Error> {
        let entries = self.hash_internal()?;
        let manifest = self.hash_directory(entries)?;
        Ok(manifest)
    }

    fn hash_internal<C: FromParallelIterator<Entry>>(&self) -> Result<C, Error> {
        // Ensure that we never include a leading `/` or `\` when stripping paths.
        let prefix_len = if self.directory_path.as_str().ends_with('/')
            || self.directory_path.as_str().ends_with('\\')
        {
            self.directory_path.as_str().len()
        } else {
            self.directory_path.as_str().len() + 1
        };
        let file_list = self.find_files(prefix_len)?;
        let entries = self.hash_files(file_list, prefix_len)?;
        Ok(entries)
    }

    fn find_files(&self, prefix_len: usize) -> Result<Vec<Utf8PathBuf>, Error> {
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileDiscoveryStarted)?;
        }
        let mut file_list = FileFinder::from(self).find()?;
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileDiscoveryCompleted(file_list.len()))?;
        }
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

    fn hash_files<C: FromParallelIterator<Entry>>(
        &self,
        file_list: Vec<Utf8PathBuf>,
        prefix_len: usize,
    ) -> Result<C, Error> {
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileHashingStarted)?;
        }
        let ret = file_list
            .into_par_iter()
            .map_with(self, |s, file| {
                if let Some(b) = &s.cancel_flag
                    && b.load(Ordering::Relaxed)
                {
                    return Err(Error::Cancelled);
                }
                let mut hasher = Hasher::new();
                let reader = fs::File::open(file.as_std_path())?;
                // Calls to `Hasher::update_reader` internally buffer 64KiB of
                // data, so we don't need to worry about doing that manually.
                hasher.update_reader(reader)?;
                // SAFETY: Since all files are descendants of dir_path,
                // they all must have dir_path as a prefix.
                let stripped_file_path = unsafe { file.as_str().get_unchecked(prefix_len..) };
                let path = oi_vei(stripped_file_path);
                let hash = hasher.finalize();
                let size = hasher.count();
                if let Some(sender) = &s.progress_channel {
                    sender.send(Event::FileHashed(file))?;
                }
                Ok(Entry { path, hash, size })
            })
            .collect();
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileHashingCompleted)?;
        }
        ret
    }

    fn hash_directory(self, entries: Vec<Entry>) -> Result<Manifest, SendError<Event>> {
        let directory_name = self
            .directory_path
            .file_name()
            .unwrap_or(self.directory_path.as_str())
            .to_string();
        let mut hasher = Hasher::new();
        let mut directory_size = 0;
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::DirectoryHashingStarted)?;
        }
        for entry in &entries {
            hasher.update(entry.hash.as_bytes());
            directory_size += entry.size;
        }
        let directory_hash = hasher.finalize();
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::DirectoryHashingCompleted)?;
        }
        Ok(Manifest {
            version: MANIFEST_VERSION,
            directory_name,
            directory_hash,
            directory_size,
            entries,
        })
    }

    // pub fn verify(&self) -> Result<Result<(), Vec<Entry>>, Error> {
    //     let old_data = fs::read(HASHFILE)?;
    //     let (new_entries, old_manifest) = rayon::join(
    //         || self.hash_internal::<HashSet<Entry>>(),
    //         || serde_json::from_slice::<Manifest>(&old_data),
    //     );
    //     let old_entries = old_manifest?.entries;
    //     let new_entries = new_entries?;
    //     let missing_entries = old_entries
    //         .into_iter()
    //         .filter(|entry| !new_entries.contains(entry))
    //         .collect::<Vec<Entry>>();
    //     match missing_entries.len() {
    //         // OK OK, I'M NOT OK
    //         0 => Ok(Ok(())),
    //         _ => Ok(Err(missing_entries)),
    //     }
    // }

    // pub fn verify_specific(&self, _entries: Vec<Entry>) -> Result<Result<(), Vec<Entry>>, Error> {
    //     todo!()
    // }
}

/// Windows always has to be so funny and unique >:(
#[inline]
fn oi_vei(s: &str) -> Utf8PathBuf {
    if cfg!(windows) {
        // Codegen for this shit is actually insanely good. Also fuck windows.
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
    .into()
}
