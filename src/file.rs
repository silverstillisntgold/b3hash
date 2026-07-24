use crate::{HASHFILE, hasher::DirectoryHasher};
use camino::Utf8PathBuf;
use parking_lot::Mutex;
use rayon::Scope;
use std::io;

const ERROR_CAPACITY: usize = 1 << 2;
const FILE_CAPACITY_GLOBAL: usize = 1 << 20;
const FILE_CAPACITY_LOCAL: usize = 1 << 10;
const HIDDEN_ENTRY_PREFIX: char = '.';

/// Utility macro so I don't have to retype this shit.
macro_rules! unwrap_or_push_error_and_return {
    ($fallible_expr: expr, $errors: expr) => {
        match ($fallible_expr) {
            Ok(v) => v,
            Err(e) => {
                ($errors).lock().push(e);
                return;
            }
        }
    };
}

/// Utility struct for recursively finding all files within a directory.
pub struct FileFinder<'a> {
    directory_hasher: &'a DirectoryHasher,
    errors: Mutex<Vec<io::Error>>,
    paths: Mutex<Vec<Utf8PathBuf>>,
}

impl<'a> From<&'a DirectoryHasher> for FileFinder<'a> {
    fn from(value: &'a DirectoryHasher) -> Self {
        Self {
            directory_hasher: value,
            errors: Mutex::new(Vec::with_capacity(ERROR_CAPACITY)),
            paths: Mutex::new(Vec::with_capacity(FILE_CAPACITY_GLOBAL)),
        }
    }
}

impl<'a> FileFinder<'a> {
    /// Returns an unsorted list of all files within the specified directory.
    #[inline(never)]
    pub fn find(self) -> Result<Vec<Utf8PathBuf>, io::Error> {
        let root_dir_path = self.directory_hasher.directory_path.clone();
        rayon::in_place_scope(|scope| self.recurse_directory(scope, root_dir_path));
        // If any errors were found, we only propagate the first.
        match self.errors.into_inner().into_iter().next() {
            None => Ok(self.paths.into_inner()),
            Some(e) => Err(e),
        }
    }

    /// Iterates over all entries in `dir_path`.
    ///
    /// Files are appended to the internal `self.paths` buffer.
    ///
    /// Directories spawn new instances of `recurse_directory` with themselves as `dir_path`.
    ///
    /// Symlinks are ignored (fuck symlinks).
    fn recurse_directory(&'a self, scope: &Scope<'a>, dir_path: Utf8PathBuf) {
        // Only checked once to minimize lock contention.
        if !self.errors.lock().is_empty() {
            return;
        }
        let entries = unwrap_or_push_error_and_return!(dir_path.read_dir_utf8(), self.errors);
        // Per-directory buffer so `self.paths` only needs to be locked once.
        let mut paths_local = Vec::with_capacity(FILE_CAPACITY_LOCAL);
        for entry in entries {
            let entry = unwrap_or_push_error_and_return!(entry, self.errors);
            if self.should_skip(entry.file_name()) {
                continue;
            }
            let file_type = unwrap_or_push_error_and_return!(entry.file_type(), self.errors);
            // We store paths instead of entries because entries can be
            // fucking massive (632 bytes on Windows), but paths are always
            // just a fancy wrapper for an underlying vector (32 bytes).
            let path = entry.into_path();
            if file_type.is_file() {
                paths_local.push(path);
            } else if file_type.is_dir() {
                scope.spawn(|new_scope| self.recurse_directory(new_scope, path));
            } else {
                if self.directory_hasher.follow_symlinks {
                    let path =
                        unwrap_or_push_error_and_return!(path.canonicalize_utf8(), self.errors);
                    let metadata = unwrap_or_push_error_and_return!(path.metadata(), self.errors);
                    if metadata.is_file() {
                        paths_local.push(path);
                    } else if metadata.is_dir() {
                        scope.spawn(|new_scope| self.recurse_directory(new_scope, path));
                    } else {
                        unreachable!("we've already resolved the symlink");
                    }
                }
            }
        }
        // Avoid locking `self.paths` when current `dir_path` only contains directories.
        if !paths_local.is_empty() {
            self.paths.lock().extend(paths_local);
        }
    }

    /// What do you think it does lol.
    #[inline]
    fn should_skip(&self, file_name: &str) -> bool {
        const {
            assert!(
                HASHFILE.as_bytes()[0] == HIDDEN_ENTRY_PREFIX as u8,
                "we're operating on the assumption that \"HASHFILE\" is hidden"
            );
        }
        if self.directory_hasher.respect_hidden {
            // The hashfile itself is hidden, so there's no need to explicitly
            // check for it when we're already respecting hidden entries.
            file_name.starts_with(HIDDEN_ENTRY_PREFIX)
        } else {
            file_name.eq(HASHFILE)
        }
    }
}
