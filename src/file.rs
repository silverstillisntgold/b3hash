use crate::HASHFILE;
use crate::hasher::DirectoryHasher;
use crate::util::Error;
use camino::Utf8PathBuf;
use parking_lot::Mutex;
use rayon::Scope;

const ERROR_CAP_DEFAULT: usize = 1 << 2;
const FILE_CAP_DEFAULT_GLOBAL: usize = 1 << 20;
const FILE_CAP_DEFAULT_LOCAL: usize = 1 << 10;
const HIDDEN_ENTRY_PREFIX: char = '.';

/// Utility struct for recursively finding all files within a directory.
pub struct FileFinder<'a> {
    directory_hasher: &'a DirectoryHasher,
    errors: Mutex<Vec<Error>>,
    paths: Mutex<Vec<Utf8PathBuf>>,
}

impl<'a> From<&'a DirectoryHasher> for FileFinder<'a> {
    fn from(value: &'a DirectoryHasher) -> Self {
        let errors = Mutex::new(Vec::with_capacity(ERROR_CAP_DEFAULT));
        let paths = Mutex::new(Vec::with_capacity(FILE_CAP_DEFAULT_GLOBAL));
        Self {
            directory_hasher: value,
            errors,
            paths,
        }
    }
}

/// Utility macro so I don't have to retype this shit.
macro_rules! unwrap_or_push_error_and_return {
    ($expr: expr, $errors: ident) => {
        match $expr {
            Ok(value) => value,
            Err(e) => {
                $errors.lock().push(e.into());
                return;
            }
        }
    };
}

impl<'a> FileFinder<'a> {
    /// Returns a list of all visible files within the directory specified.
    #[inline(never)]
    pub fn find(self) -> Result<Vec<Utf8PathBuf>, Error> {
        let root_dir_path = self.directory_hasher.directory_path.clone();
        rayon::in_place_scope(|scope| self.recurse_directory(scope, root_dir_path));
        // If any errors were found, we only propagate the first.
        match self.errors.into_inner().into_iter().next() {
            None => Ok(self.paths.into_inner()),
            Some(e) => Err(e),
        }
    }

    /// For directory `dir_path`, sends all file paths into `paths`, spawns a new parallel instance for
    /// all directories, and sends any errors encountered into `errors`. Newly spawned instances will
    /// terminate immediately if `errors` contains any errors, but will finish working within their current
    /// directory if an error is pushed in some other worker during their execution.
    fn recurse_directory(&'a self, scope: &Scope<'a>, dir_path: Utf8PathBuf) {
        let errors = &self.errors;
        // Kill procedure early if an error has already been encountered.
        // Only check once to avoid excessive lock contention.
        if !errors.lock().is_empty() {
            return;
        }
        let entries = unwrap_or_push_error_and_return!(dir_path.read_dir_utf8(), errors);
        // Every time we need to operate on `paths` we have to lock it's mutex,
        // so it's best to keep all our paths in a local vec and just append
        // them in bulk after all entries have been scanned.
        let mut paths_local = Vec::with_capacity(FILE_CAP_DEFAULT_LOCAL);
        for entry in entries {
            let entry = unwrap_or_push_error_and_return!(entry, errors);
            let entry_name = entry.file_name();
            if (self.directory_hasher.respect_hidden && entry_name.starts_with(HIDDEN_ENTRY_PREFIX))
                || entry_name == HASHFILE
            {
                continue;
            }
            let metadata = unwrap_or_push_error_and_return!(entry.metadata(), errors);
            // Prefer to use `Utf8PathBuf` because `Utf8DirEntry` contains things we don't have
            // any use for, and it's absolutely massive on windows platforms.
            let path = entry.into_path();
            if metadata.is_file() {
                paths_local.push(path);
            } else if metadata.is_dir() {
                scope.spawn(|new_scope| self.recurse_directory(new_scope, path));
            }
        }
        self.paths.lock().extend(paths_local);
    }
}
