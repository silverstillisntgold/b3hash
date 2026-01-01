use crate::MANIFEST_VERSION;
use crate::file::FileFinder;
use crate::manifest::*;
use crate::util::*;
use blake3::Hasher;
use bon::Builder;
use camino::Utf8PathBuf;
use crossbeam_channel::Sender;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;

/// Windows always has to be so funny and unique >:(
fn fuck_windows(s: &str) -> Utf8PathBuf {
    if cfg!(windows) {
        // Codegen for this shit is actually insanely good.
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
    .into()
}

#[derive(Builder, Debug, Deserialize, Serialize)]
pub struct DirectoryHasher {
    /// Specifies the directory which will be hashed.
    pub(crate) directory_path: Utf8PathBuf,

    #[builder(default = true)]
    pub(crate) respect_hidden: bool,

    #[serde(skip)]
    pub(crate) progress_channel: Option<Sender<Event>>,

    #[builder(skip)]
    #[serde(skip)]
    pub(crate) cancel_handle: Option<CancelHandle>,
}

impl DirectoryHasher {
    /// Attaches a [`CancelHandle`] to `self` for mid-process cancellation.
    /// Calling this multiple times will drop and override previous handles.
    pub fn cancel_handle(&mut self) -> CancelHandle {
        let cancel_handle = CancelHandle::default();
        self.cancel_handle = Some(cancel_handle.clone());
        cancel_handle
    }

    /// Consumes `self` to hash the contents of the given directory and return
    /// the resulting [`Manifest`], or an [`Error`] if one is encountered.
    #[inline(never)]
    pub fn hash(self) -> Result<Manifest, Error> {
        let entries = self.hash_entries()?;
        let manifest = self.hash_directory(entries)?;
        Ok(manifest)
    }

    pub(crate) fn hash_entries(&self) -> Result<Vec<Entry>, Error> {
        let file_list = self.find_files()?;
        let entries = self.hash_files(file_list)?;
        Ok(entries)
    }

    fn find_files(&self) -> Result<Vec<Utf8PathBuf>, Error> {
        let prefix_len = self.prefix_len();
        let mut file_list = FileFinder::from(self).find()?;
        // Stable sorting has no use here since all paths are unique.
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

    fn hash_files(&self, file_list: Vec<Utf8PathBuf>) -> Result<Vec<Entry>, Error> {
        let prefix_len = self.prefix_len();
        file_list
            .into_par_iter()
            .map(|entry| {
                if let Some(cancel_handle) = &self.cancel_handle
                    && cancel_handle.load()
                {
                    return Err(Error::Cancelled);
                }
                let mut hasher = Hasher::new();
                let reader = fs::File::open(entry.as_std_path())?;
                hasher.update_reader(reader)?;
                // SAFETY: Since all files are descendants of dir_path,
                // they all must have dir_path as a prefix.
                let stripped_file_path = unsafe { entry.as_str().get_unchecked(prefix_len..) };
                let path = fuck_windows(stripped_file_path);
                let hash = hasher.finalize();
                // Because we've only hashed a single file, the amount of bytes
                // hashed represents the size of the file hashed.
                let size = hasher.count();
                if let Some(tx) = &self.progress_channel {
                    tx.send(Event::FileHashed(entry))?;
                }
                Ok(Entry { path, hash, size })
            })
            .collect()
    }

    fn hash_directory(mut self, entries: Vec<Entry>) -> Result<Manifest, Error> {
        let directory_name = self
            .directory_path
            .file_name()
            .unwrap_or(self.directory_path.as_str())
            .to_string();
        let mut hasher = Hasher::new();
        let mut directory_size = 0;
        // There are faster ways to do this, but this simple and in-place approach is preferred.
        for entry in &entries {
            hasher.update(entry.path.as_str().as_bytes());
            hasher.update(entry.hash.as_bytes());
            hasher.update(&entry.size.to_le_bytes());
            directory_size += entry.size;
        }
        let directory_hash = hasher.finalize();
        // Make sure the old channel/handle are closed/dropped.
        self.progress_channel = None;
        self.cancel_handle = None;
        Ok(Manifest {
            version: MANIFEST_VERSION,
            directory_name,
            directory_hash,
            directory_size,
            entries,
            directory_hasher: self,
        })
    }

    /// Ensures that we never include a leading `/` or `\` when stripping paths.
    fn prefix_len(&self) -> usize {
        let s = self.directory_path.as_str();
        if s.ends_with('/') || s.ends_with('\\') {
            s.len()
        } else {
            s.len() + 1
        }
    }
}
