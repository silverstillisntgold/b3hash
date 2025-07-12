use crate::IOResult;
use camino::Utf8PathBuf;
use std::io;
use std::sync::{Arc, Mutex};

const CAPACITY_ERRORS: usize = 1 << 4;
const CAPACITY_PATHS: usize = 1 << 20;
const CAPACITY_TMP_PATHS: usize = 1 << 8;
const HIDDEN_ENTRY_PREFIX: char = '.';

struct ArcVec<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> Clone for ArcVec<T> {
    #[inline]
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<T> ArcVec<T> {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let inner = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
        Self { inner }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        self.inner.lock().unwrap().push(value);
    }

    #[inline]
    pub fn extend(&mut self, iter: impl IntoIterator<Item = T>) {
        self.inner.lock().unwrap().extend(iter);
    }

    #[inline]
    pub fn into_inner(self) -> Vec<T> {
        Arc::into_inner(self.inner).unwrap().into_inner().unwrap()
    }
}

/// Build a `Vec` containing the paths of all visible files within `dir_path`.
///
/// This is done in such a way that each directory spawns a distinct rayon task,
/// containing it's own sequential iterator over it's contents. Valid file paths are sent
/// through a channel to later be collected into the returned `Vec`. If an error is
/// encountered, each rayon task will terminate as soon as possible and the error
/// will be propagated to the caller.
///
/// The ordering of paths in the returned `Vec` is non-deterministic.
#[inline(never)]
pub fn get_file_paths(dir_path: Utf8PathBuf) -> IOResult<Vec<Utf8PathBuf>> {
    let errors = ArcVec::with_capacity(CAPACITY_ERRORS);
    let paths = ArcVec::with_capacity(CAPACITY_PATHS);

    let errors_clone = errors.clone();
    let paths_clone = paths.clone();
    rayon::in_place_scope(move |scope| {
        this_is_a_gyatt_function(dir_path, errors_clone, paths_clone, scope);
    });

    // If there are any errors, we only worry about the first one.
    match errors.into_inner().into_iter().nth(0) {
        None => Ok(paths.into_inner()),
        Some(e) => Err(e),
    }
}

macro_rules! unwrap_or_push_error {
    ($expr: expr, $err_chan: ident) => {
        match $expr {
            Ok(value) => value,
            Err(e) => {
                $err_chan.push(e);
                return;
            }
        }
    };
}

fn this_is_a_gyatt_function(
    dir_path: Utf8PathBuf,
    mut errors: ArcVec<io::Error>,
    mut paths: ArcVec<Utf8PathBuf>,
    scope: &rayon::Scope,
) {
    // Terminate early if some other worker already pushed an error.
    if !errors.is_empty() {
        return;
    }
    let entries = unwrap_or_push_error!(dir_path.read_dir_utf8(), errors);
    // We push all file paths in the current directory into a local array,
    // then before exiting the function we move this data to our shared `paths`.
    // This, along with the initial `VecArc::is_empty` check, means that
    // we only ever lock each of the `ArcVec` instances once per function.
    let mut tmp_paths = Vec::with_capacity(CAPACITY_TMP_PATHS);
    for entry in entries {
        let entry = unwrap_or_push_error!(entry, errors);
        if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
            continue;
        }
        let metadata = unwrap_or_push_error!(entry.metadata(), errors);
        let path = entry.into_path();
        if metadata.is_file() {
            if metadata.len() > 0 {
                tmp_paths.push(path);
            }
        } else if metadata.is_dir() {
            let errors_clone = errors.clone();
            let paths_clone = paths.clone();
            scope.spawn(move |new_scope| {
                this_is_a_gyatt_function(path, errors_clone, paths_clone, new_scope);
            });
        }
    }
    paths.extend(tmp_paths);
}
