use crate::{
    HashingError,
    file::FileFinder,
    hash_entries,
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

/// Iterator over the paths of all files hashed by the source [`DirectoryHasher`].
///
/// File paths are relative to the root directory, and the order in which they are received is non-deterministic.
///
/// # Warning
///
/// Because this type contains a [`JoinHandle`], dropping it mid-process causes the handle to become detached.
/// The iterator should generally be consumed with [`Self::into_manifest`] or [`Self::into_manifest_with`].
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
        // Because we never unwrap/expect anywhere else, this should only panic when
        // we have an underlying library/OS failure, which is out of our control.
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
    /// Hashes the contents of the specified directory and returns the resulting [`Manifest`].
    #[inline(never)]
    pub fn hash(mut self) -> Result<Manifest, HashingError> {
        // Canonicalize to avoid any potential issues with relative paths.
        self.directory_path = self.directory_path.canonicalize_utf8()?;
        let file_list = FileFinder::from(&self).find()?;
        let mut entries = self.hash_files(file_list)?;
        // Because we've canonicalized the directory path, all the elements in `file_list` will also
        // have canonicalized paths, which increases the number of components in each `Utf8PathBuf`.
        // The comparison of `Utf8PathBuf` operates on it's components, so more components means the
        // comparison takes longer. And having fully canonicalized paths makes it as long as possible.
        // Doing sorting here means we are comparing the paths after they've been stripped of
        // their common prefix (directory path and all it's parents), which makes comparison much faster.
        entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
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
                    return Err(HashingError::Canceled);
                }
                let mut hasher = Hasher::new();
                // We want to use a reader here because we will likely be reading many large
                // files at once. If we were to load them into memory we'd quickly run out
                // and probably crash the system, and using memory mapping makes the system
                // extremely unresponsive. The buffered direct file reading provided by blake3
                // is a perfect middle ground for our implementation.
                let reader = fs::File::open(file_path.as_std_path())?;
                hasher.update_reader(reader)?;
                let path = file_path
                    .strip_prefix(self.directory_path.as_path())
                    .map(Utf8Path::to_path_buf)
                    .expect("all file paths should be children of `self.directory_path`");
                let hash = hasher.finalize();
                // Because we've only hashed a single file, the number of
                // bytes hashed represents the size of the file in bytes.
                let size = hasher.count();
                if let Some(tx) = &self.progress_channel {
                    // If this would propagate an error, we've already canceled hashing and
                    // returned the appropriate error, so we can ignore this one.
                    let _ = tx.send(path.clone());
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
        let directory_path = Some(self.directory_path);
        let (directory_hash, directory_size) = hash_entries(&entries);
        Ok(Manifest {
            directory_path,
            directory_name,
            directory_hash,
            directory_size,
            entries,
        })
    }

    /// Attaches an internal cancel handle to `self` for early cancelation of file hashing.
    fn cancel_handle(&mut self) -> Arc<AtomicBool> {
        let cancel_handle = Arc::new(AtomicBool::new(false));
        self.cancel_handle = Some(cancel_handle.clone());
        cancel_handle
    }
}
