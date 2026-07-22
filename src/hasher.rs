use crate::{
    HashingError,
    file::FileFinder,
    manifest::{Entry, Manifest},
};
use blake3::Hasher;
use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

/// Windows always has to be so funny and unique >:(
#[inline]
fn fuck_windows(s: &str) -> Utf8PathBuf {
    if cfg!(windows) {
        // Codegen for this shit is actually insanely good.
        s.replace('\\', "/")
    } else {
        s.to_owned()
    }
    .into() // This conversion is free.
}

/// Iterator over the paths of all files hashed by the source [`DirectoryHasher`].
///
/// File paths are relative to the root directory, and the order in which they are received is non-deterministic.
///
/// # Warning
///
/// Because this type contains a [`JoinHandle`], dropping it mid-process causes the handle to become detached.
/// The iterator should generally be consumed with one of [`Self::into_manifest`] or [`Self::into_manifest_with`].
///
/// If you are using standard [`Iterator`] functions to consume the iterator, you **must** call
/// [`Self::cancel`] or [`Self::into_manifest`] after the iterator is exhausted.
///
/// # Examples
///
/// ```ignore
/// use camino::Utf8PathBuf;
/// use b3hash::{DirectoryHasher, DirectoryHasherIter, Manifest};
///
/// let path_to_dir_root: Utf8PathBuf = get_path_for_hashing();
/// let mut hasher_iter: DirectoryHasherIter = DirectoryHasher::builder()
///     .directory_path(path_to_dir_root)
///     .build()
///     .into_iter();
/// let manifest: Manifest = hasher_iter
///     .into_manifest_with(|file_path: Utf8PathBuf| {
///         // Do something with `file_path`.
///     })
///     .unwrap(); // <-- Probably want to handle this error.
/// ```
pub struct DirectoryHasherIter {
    cancel_handle: Arc<AtomicBool>,
    manifest_handle: JoinHandle<Result<Manifest, HashingError>>,
    rx: Receiver<Utf8PathBuf>,
}

impl Iterator for DirectoryHasherIter {
    type Item = Utf8PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        self.rx.recv().ok()
    }
}

impl DirectoryHasherIter {
    /// Cancels file hashing (if any files remain) and closes the backing thread,
    /// discarding the resulting [`HashingError::Canceled`].
    ///
    /// Note that this method may block while any in-progress file hashing completes.
    pub fn cancel(self) {
        self.cancel_handle.store(true, Ordering::Relaxed);
        // Make sure the thread is joined, otherwise it ends up detached.
        let _ = self.into_manifest();
    }

    /// Consumes the remainder of the iterator and returns the resulting [`Manifest`].
    pub fn into_manifest(self) -> Result<Manifest, HashingError> {
        self.into_manifest_with(|_| {})
    }

    /// Consumes the remainder of the iterator and returns the resulting [`Manifest`],
    /// performing function `f` on all paths received from the iterator.
    pub fn into_manifest_with<F>(mut self, f: F) -> Result<Manifest, HashingError>
    where
        F: Fn(Utf8PathBuf),
    {
        for path in &mut self {
            f(path);
        }
        // Because we never unwrap/expect anywhere else, this should only fail when
        // we have an underlying library failure, which we can't handle anyway.
        self.manifest_handle.join().unwrap()
    }
}

/// Struct for hashing directory trees, which should only ever be constructed with [`Self::builder`].
///
/// The only required field is `directory_path`, which specifies the directory whose contents
/// should be hashed. By default, hidden files and directories will be ignored, but this can be
/// modified with [`DirectoryHasherBuilder::respect_hidden`].
///
/// If you would like to monitor which files are being hashed, the [`IntoIterator`]
/// implementation provides a [`DirectoryHasherIter`], which makes it easy to process the
/// resulting file paths as files are hashed.
///
/// # Examples
///
/// ```ignore
/// use camino::Utf8PathBuf;
/// use b3hash::{DirectoryHasher, Manifest};
///
/// let path_to_dir_root: Utf8PathBuf = get_path_for_hashing();
/// let hasher: DirectoryHasher = DirectoryHasher::builder()
///     .directory_path(path_to_dir_root)
///     .build();
/// let manifest: Manifest = hasher.hash().unwrap(); // <-- Probably want to handle this error.
/// ```
#[derive(bon::Builder)]
pub struct DirectoryHasher {
    /// Path to the directory that will be hashed.
    pub(crate) directory_path: Utf8PathBuf,

    /// Contains the length of `directory_path` when it is the leading
    /// component of a file or directory beneath it.
    #[builder(skip)]
    prefix_len: usize,

    /// Should files and directories beginning with `.` be skipped?
    #[builder(default = true)]
    pub(crate) respect_hidden: bool,

    /// Optional [`SyncSender`] for sending paths of hashed files to a [`Receiver`].
    ///
    /// The order in which file paths are sent over this channel is non-deterministic.
    #[builder(skip)]
    progress_channel: Option<SyncSender<Utf8PathBuf>>,

    /// Optional cancel handle for canceling hashing operation early from outside.
    ///
    /// Must be created using [`Self::cancel_handle`].
    #[builder(skip)]
    cancel_handle: Option<Arc<AtomicBool>>,
}

impl IntoIterator for DirectoryHasher {
    type Item = Utf8PathBuf;
    type IntoIter = DirectoryHasherIter;

    fn into_iter(mut self) -> Self::IntoIter {
        // Using a bounded, 0-length channel so the backing computation
        // thread only progresses when calling `next` on the iterator.
        let (tx, rx) = mpsc::sync_channel(0);
        self.progress_channel = Some(tx);
        let cancel_handle = self.cancel_handle();
        let manifest_handle = thread::spawn(|| self.hash());
        Self::IntoIter {
            cancel_handle,
            manifest_handle,
            rx,
        }
    }
}

impl DirectoryHasher {
    /// Consumes `self` to hash the contents of the given directory and returns the resulting [`Manifest`].
    #[inline(never)]
    pub fn hash(mut self) -> Result<Manifest, HashingError> {
        // Canonicalize here so we always have the correct name of the directory being hashed.
        self.directory_path = self.directory_path.canonicalize_utf8()?;
        // This ensures that we never include a leading `/` or `\` when stripping paths.
        // Need to do this here since we've just altered `self.directory_path`.
        //
        // path/  --> len == 5
        // 012345 --> we want to start at 5 to avoid the slash
        //
        // path   --> len == 4
        // 012345 --> we want to start at 5 to avoid the slash that deeper paths will add
        self.prefix_len = {
            let s = self.directory_path.as_str();
            if s.ends_with('/') || s.ends_with('\\') {
                s.len()
            } else {
                s.len() + 1
            }
        };
        let mut file_list = FileFinder::from(&self).find()?;
        // Stable sorting has no use here because file paths are inherently unique.
        file_list.sort_unstable_by(|a, b| {
            // We don't know how long the root directory prefix will be, so it's best
            // to strip it out to minimize the time spent comparing elements.
            let a_stripped = self.strip_prefix(a.as_path());
            let b_stripped = self.strip_prefix(b.as_path());
            a_stripped.cmp(b_stripped)
        });
        let entries = self.hash_files(file_list)?;
        self.hash_directory(entries)
    }

    /// Maps all items in `file_list` from [`Utf8PathBuf`] to [`Entry`] by hashing the file located at each path.
    fn hash_files(&self, file_list: Vec<Utf8PathBuf>) -> Result<Vec<Entry>, HashingError> {
        file_list
            .into_par_iter()
            .map(|file_path| {
                if let Some(cancel_handle) = &self.cancel_handle
                    && cancel_handle.load(Ordering::Relaxed)
                {
                    std::hint::cold_path();
                    return Err(HashingError::Canceled);
                }
                let mut hasher = Hasher::new();
                let reader = fs::File::open(file_path.as_std_path())?;
                hasher.update_reader(reader)?;
                let path = fuck_windows(self.strip_prefix(file_path.as_path()));
                let hash = hasher.finalize();
                // Because we've only hashed a single file, the amount of
                // bytes hashed represents the size of the file hashed.
                let size = hasher.count();
                if let Some(tx) = &self.progress_channel {
                    // If this would propagate an error, we've already canceled hashing and
                    // returned the appropriate error, so we can ignore this one.
                    let _ = tx.send(file_path);
                }
                Ok(Entry { path, hash, size })
            })
            .collect()
    }

    /// Processes `entries` into a [`Manifest`] by hashing all fields of each [`Entry`] in order.
    ///
    /// # Warning
    ///
    /// It is expected that `entries` is sorted by file path, so that the ordering is consistent,
    /// otherwise the directory hash in the returned [`Manifest`] will be different each time.
    fn hash_directory(self, entries: Vec<Entry>) -> Result<Manifest, HashingError> {
        let directory_name = self
            .directory_path
            .file_name()
            .unwrap_or(self.directory_path.as_str())
            .to_owned();
        let mut hasher = Hasher::new();
        let mut directory_size = 0;
        // There are faster ways to do this, but this simple and non-allocating approach is preferred.
        // We want to hash all contents of each entry, so **any** small change to
        // an entry is reflected in the final directory hash.
        for entry in &entries {
            // WARNING: Changing the order in which these fields are fed to
            // the hasher will change the final value of `directory_hash`.
            hasher.update(entry.path.as_str().as_bytes());
            hasher.update(entry.hash.as_bytes());
            hasher.update(&entry.size.to_le_bytes());
            directory_size += entry.size;
        }
        let directory_hash = hasher.finalize();
        Ok(Manifest {
            directory_path: Some(self.directory_path),
            directory_name,
            directory_hash,
            directory_size,
            entries,
        })
    }

    /// Strips the root directory prefix (including it's trailing slash) from `path`,
    /// returning the child path relative to the root directory.
    #[inline]
    fn strip_prefix<'a>(&self, path: &'a Utf8Path) -> &'a str {
        // SAFETY: Because all files are descendants of `self.directory_path`,
        // they all must have it as a prefix. And because `self.prefix_len`
        // holds the index which is the start of the relative path, this will
        // always return the entire relative path without the root directory.
        unsafe { path.as_str().get_unchecked(self.prefix_len..) }
    }

    /// Attaches an internal cancel handle to `self` for early cancelation of file hashing.
    fn cancel_handle(&mut self) -> Arc<AtomicBool> {
        let cancel_handle = Arc::new(AtomicBool::new(false));
        self.cancel_handle = Some(cancel_handle.clone());
        cancel_handle
    }
}
