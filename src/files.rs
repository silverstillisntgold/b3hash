use crate::arcvec::ArcVec;
use camino::{Utf8Path, Utf8PathBuf};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::{fs, io};

const CAPACITY_PATHS: usize = 1 << 20;
const CAPACITY_TMP_PATHS: usize = 1 << 8;
const HIDDEN_ENTRY_PREFIX: char = '.';
const IGNOREFILE_COMMENT: char = '#';
const IGNOREFILE_TARGET: &str = ".gitignore";

/// Constructs a [`GlobSet`] for ignoring files/directories using the local ignore file.
fn get_ignore_list(dir_path: &Utf8Path) -> io::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let ignore_path = dir_path.join(IGNOREFILE_TARGET);
    match fs::read_to_string(ignore_path) {
        Ok(s) => {
            s.trim()
                .lines()
                .map(str::trim)
                .filter(|s| s.chars().next().is_some_and(|s| s != IGNOREFILE_COMMENT))
                .for_each(|glob| {
                    // Ignore glob building failures.
                    if let Ok(pat) = Glob::new(glob) {
                        builder.add(pat);
                    }
                });
        }
        Err(e) => match e.kind() {
            // It's fine if there isn't an ignore file.
            io::ErrorKind::NotFound => (),
            _ => return Err(e),
        },
    };
    Ok(builder.build().unwrap_or_default())
}

/// Builds a `Vec` containing the paths of all visible files within `dir_path`.
///
/// The ordering of paths in the returned `Vec` is non-deterministic.
#[inline(never)]
pub fn get_file_paths(dir_path: Utf8PathBuf) -> io::Result<Vec<Utf8PathBuf>> {
    let errors = ArcVec::new();
    let ignore_list = get_ignore_list(&dir_path)?;
    let paths = ArcVec::with_capacity(CAPACITY_PATHS);

    let errors_clone = errors.clone();
    let ignore_list_ref = &ignore_list;
    let paths_clone = paths.clone();
    rayon::in_place_scope(move |scope| {
        this_is_a_gyatt_function(dir_path, errors_clone, ignore_list_ref, paths_clone, scope);
    });

    // If there are any errors, we only care about the first one.
    match errors.into_inner().into_iter().next() {
        None => Ok(paths.into_inner()),
        Some(e) => Err(e),
    }
}

macro_rules! unwrap_or_push_error {
    ($expr: expr, $err_chan_desu: ident) => {
        match $expr {
            Ok(value) => value,
            Err(e) => {
                $err_chan_desu.push(e);
                return;
            }
        }
    };
}

/// Pushes all file paths within `dir_path` into `paths` and spawns new parallel
/// instances of itself for each subdirectory.
///
/// If any errors are encountered they're appended to `errors` and no further
/// directories will be scanned.
#[inline(never)]
fn this_is_a_gyatt_function<'a>(
    dir_path: Utf8PathBuf,
    errors: ArcVec<io::Error>,
    ignore_list: &'a GlobSet,
    paths: ArcVec<Utf8PathBuf>,
    scope: &rayon::Scope<'a>,
) {
    // Terminate early if some other worker has already pushed an error.
    if !errors.is_empty() {
        return;
    }
    let entries = unwrap_or_push_error!(dir_path.read_dir_utf8(), errors);
    // Each directory maintains it's own cache of file paths, which will
    // be moved into the shared `paths` after all entries in the directory
    // have been processed. This ensures that each spawned scope will only ever
    // hold a lock on `paths` once, minimizing lock contention.
    let mut tmp_paths = Vec::with_capacity(CAPACITY_TMP_PATHS);
    for entry in entries {
        let entry = unwrap_or_push_error!(entry, errors);
        if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) || ignore_list.is_match(entry.path())
        {
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
                this_is_a_gyatt_function(path, errors_clone, ignore_list, paths_clone, new_scope);
            });
        }
    }
    paths.extend(tmp_paths);
}
