use crate::file::FileFinder;
use crate::manifest::{Entry, Manifest};
use crate::util::{CancelHandle, Error, Event};
use blake3::Hasher;
use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::Sender;
use rayon::prelude::*;
use std::fs;

/// Convenience macro to send event(s) into the provided channel if it's `Some`.
#[macro_export]
macro_rules! send_if_channel {
    ($channel: expr, $($event: expr), +$(,)?) => {
        if let Some(tx) = ($channel).as_ref() {
            $(tx.send($event)?;)+
        }
    };
}

/// Windows always has to be so funny and unique >:(
#[inline]
fn fuck_windows(s: &str) -> Utf8PathBuf {
    if cfg!(windows) {
        // Codegen for this shit is actually insanely good.
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
    .into()
}

/// Struct for hashing directory trees, should be constructed using the builder pattern.
///
/// The only required field is `directory_path`.
///
/// # Examples
///
/// ```rust, no-run
/// let path: Utf8PathBuf = get_path_for_hashing();
/// let hasher = DirectoryHasher::builder()
///                 .directory_path(path)
///                 .build();
/// let manifest = hasher.hash().unwrap();
/// ```
#[derive(bon::Builder)]
pub struct DirectoryHasher {
    /// Path of the directory which will be hashed.
    pub(crate) directory_path: Utf8PathBuf,

    /// Contains the length of `directory_path` when it is the leading
    /// component of a file or directory beneath it.
    ///
    /// This ensures that we never include a leading `/` or `\` when stripping paths.
    ///
    /// # Examples
    ///
    /// ```text
    /// path/  --> len == 5
    /// 012345 --> we want to start at 5 to avoid the slash
    ///
    /// path   --> len == 4
    /// 012345 --> we want to start at 5 to avoid the slash that deeper paths will add
    /// ```
    #[builder(skip = {
        let s = directory_path.as_str();
        if s.ends_with('/') || s.ends_with('\\') {
            s.len()
        } else {
            s.len() + 1
        }
    })]
    prefix_len: usize,

    /// Should files and directories beginning with `.` be skipped?
    #[builder(default = true)]
    pub(crate) respect_hidden: bool,

    /// Optional [`crossbeam_channel::Sender`] for sending internally generated
    /// instances of [`Event`] to user-held [`crossbeam_channel::Receiver`].
    pub(crate) progress_channel: Option<Sender<Event>>,

    /// Optional [`CancelHandle`] for cancelling hashing operation early from outside.
    ///
    /// Must be created using [`Self::cancel_handle`].
    #[builder(skip)]
    cancel_handle: Option<CancelHandle>,
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
        self.hash_directory(entries)
    }

    /// Uses the internal `directory_path` to build a list of files to be hashed,
    /// sort them, and hash them. Stops short of processing them into a [`Manifest`] to make
    /// it easy for reuse in the file verification process.
    pub(crate) fn hash_entries(&self) -> Result<Vec<Entry>, Error> {
        send_if_channel!(self.progress_channel, Event::FileDiscoveryStarted);
        let mut file_list = FileFinder::from(self).find()?;
        send_if_channel!(
            self.progress_channel,
            Event::FileDiscoveryCompleted(file_list.len()),
            Event::FileSortingStarted
        );
        // Stable sorting has no use here because file paths are unique.
        file_list.sort_unstable_by(|a, b| {
            // We don't know how long the root directory prefix will be, so it's best
            // to strip it out to minimize the time spent sorting.
            let a_stripped = self.strip_prefix(a.as_path());
            let b_stripped = self.strip_prefix(b.as_path());
            a_stripped.cmp(b_stripped)
        });
        send_if_channel!(
            self.progress_channel,
            Event::FileSortingCompleted,
            Event::FileHashingStarted
        );
        let entries = self.hash_files(file_list);
        send_if_channel!(self.progress_channel, Event::FileHashingCompleted);
        entries
    }

    /// Maps all items in `file_list` from [`Utf8PathBuf`] to [`Entry`] by
    /// hashing the file located at the target path. Can be terminated early if
    /// the user has acquired a [`CancelHandle`].
    fn hash_files(&self, file_list: Vec<Utf8PathBuf>) -> Result<Vec<Entry>, Error> {
        file_list
            .into_par_iter()
            .map(|file_path| {
                if let Some(cancel_handle) = &self.cancel_handle
                    && cancel_handle.load()
                {
                    return Err(Error::Cancelled);
                }
                let mut hasher = Hasher::new();
                let reader = fs::File::open(file_path.as_std_path())?;
                hasher.update_reader(reader)?;
                let path = fuck_windows(self.strip_prefix(file_path.as_path()));
                let hash = hasher.finalize();
                // Because we've only hashed a single file, the amount of bytes
                // hashed represents the size of the file hashed.
                let size = hasher.count();
                send_if_channel!(self.progress_channel, Event::FileHashed(file_path));
                Ok(Entry { path, hash, size })
            })
            .collect()
    }

    /// Processes `entries` into a [`Manifest`] by hashing all fields of each [`Entry`] in order.
    fn hash_directory(self, entries: Vec<Entry>) -> Result<Manifest, Error> {
        let directory_name = self
            .directory_path
            .file_name()
            .unwrap_or(self.directory_path.as_str())
            .to_string();
        let mut hasher = Hasher::new();
        let mut directory_size = 0;
        send_if_channel!(self.progress_channel, Event::DirectoryHashingStarted);
        // There are faster ways to do this, but this simple and non-allocating approach is preferred.
        for entry in &entries {
            // WARNING: Changing the order in which these fields are fed to
            // the hasher will change the final value of `directory_hash`.
            hasher.update(entry.path.as_str().as_bytes());
            hasher.update(entry.hash.as_bytes());
            hasher.update(&entry.size.to_le_bytes());
            directory_size += entry.size;
        }
        let directory_hash = hasher.finalize();
        send_if_channel!(self.progress_channel, Event::DirectoryHashingCompleted);
        Ok(Manifest {
            directory_path: Some(self.directory_path),
            directory_name,
            directory_hash,
            directory_size,
            entries: entries.into(),
        })
    }

    /// Strips the root directory prefix (including it's trailing slash) from `path`.
    #[inline]
    fn strip_prefix<'a>(&self, path: &'a Utf8Path) -> &'a str {
        // SAFETY: Since all files are descendants of `self.directory_path`,
        // they all must have it as a prefix. And because `self.prefix_len`
        // holds the index which is the start of the relative path, this will
        // always return the entire relative path without the root directory.
        unsafe { path.as_str().get_unchecked(self.prefix_len..) }
    }
}
