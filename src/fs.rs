use crate::IOResult;
use crate::arcvec::ArcVec;
use camino::Utf8PathBuf;
use std::io;

const CAPACITY_PATHS: usize = 1 << 20;
const CAPACITY_TMP_PATHS: usize = 1 << 8;
const HIDDEN_ENTRY_PREFIX: char = '.';

/// Build a `Vec` containing the paths of all visible files within `dir_path`.
///
/// This is done in such a way that each directory spawns a distinct rayon task,
/// and is in charge of handling all the files and folders within it.
/// Files are cached in a local `Vec`, then appended to a shared `ArcVec` before
/// returning, while folders are dispatched to their own scope.
/// Errors have their own `ArcVec`, and after one thread has encountered and
/// subsequently pushed an error, all future directory scopes will immediately
/// terminate and the first error found will be propaged to the caller.
///
/// The ordering of paths in the returned `Vec` is non-deterministic.
#[inline(never)]
pub fn get_file_paths(dir_path: Utf8PathBuf) -> IOResult<Vec<Utf8PathBuf>> {
    let errors = ArcVec::new();
    let paths = ArcVec::with_capacity(CAPACITY_PATHS);

    let errors_clone = errors.clone();
    let paths_clone = paths.clone();
    rayon::in_place_scope(move |scope| {
        this_is_a_gyatt_function(dir_path, errors_clone, paths_clone, scope);
    });

    // If there are any errors, we only care about the first one.
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

#[inline(never)]
fn this_is_a_gyatt_function(
    dir_path: Utf8PathBuf,
    errors: ArcVec<io::Error>,
    paths: ArcVec<Utf8PathBuf>,
    scope: &rayon::Scope,
) {
    // Terminate early if some other worker(s) already pushed an error.
    if !errors.is_empty() {
        return;
    }
    let entries = unwrap_or_push_error!(dir_path.read_dir_utf8(), errors);
    // Each directory maintains it's own cache of file paths, which will
    // be moved into the shared `paths` after all entries in this directory
    // have been handled. This ensures that each spawned scope will only ever
    // hold a lock on `paths` once, minimizing lock contention.
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
