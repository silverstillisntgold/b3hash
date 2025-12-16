use crate::MANIFEST_VERSION;
use crate::file::FileFinder;
use crate::manifest::*;
use crate::util::*;
use blake3::Hasher;
use bon::Builder;
use camino::Utf8PathBuf;
use crossbeam_channel::{SendError, Sender};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;

/// The only required field is `directory_path`.
#[derive(Builder, Debug, Deserialize, Serialize)]
pub struct DirectoryHasher {
    /// Specifies the directory which will be hashed.
    pub(crate) directory_path: Utf8PathBuf,

    pub(crate) custom_ignore_source: Option<Utf8PathBuf>,

    #[builder(default = true)]
    pub(crate) respect_hidden: bool,

    #[builder(default = true)]
    pub(crate) respect_ignore: bool,

    #[builder(default = false)]
    pub(crate) allow_missing_ignore: bool,

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
        let entries = self.hash_internal()?;
        let manifest = self.hash_directory(entries)?;
        Ok(manifest)
    }

    /// Consumes `self` to hash the contents of the given directory and return
    /// the hashed entries without processing them into a [`Manifest`].
    #[inline(never)]
    pub fn hash_entries<C: FromParallelIterator<Entry>>(self) -> Result<C, Error> {
        self.hash_internal()
    }

    pub(crate) fn hash_internal<C: FromParallelIterator<Entry>>(&self) -> Result<C, Error> {
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
            sender.send(Event::FileSortingStarted)?;
        }
        // Stable sorting has no use here since all paths will be unique.
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
        if let Some(sender) = &self.progress_channel {
            sender.send(Event::FileSortingCompleted)?;
        }
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
                if let Some(h) = &s.cancel_handle
                    && h.load()
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

    fn hash_directory(mut self, entries: Vec<Entry>) -> Result<Manifest, SendError<Event>> {
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
        // Make sure the channel/handle are closed/dropped.
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
