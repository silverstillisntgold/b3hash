use crate::arcvec::ArcVec;
use crate::{DirectoryHasher, Error, HASHFILE, IGNOREFILE};
use camino::{Utf8Path, Utf8PathBuf};
use globset::{Glob, GlobSet};
use std::fs;

const FILE_CAP_DEFAULT_GLOBAL: usize = 1 << 20;
const FILE_CAP_DEFAULT_LOCAL: usize = 1 << 10;
const HIDDEN_ENTRY_PREFIX: char = '.';
const IGNOREFILE_COMMENT: char = '#';

/// Utility struct for recursively finding all files within a given directory.
pub struct FileFinder<'a> {
    directory_path: &'a Utf8Path,
    custom_ignore_source: Option<&'a Utf8Path>,
    respect_hidden: bool,
    respect_ignore: bool,
}

impl<'a> From<&'a DirectoryHasher> for FileFinder<'a> {
    fn from(value: &'a DirectoryHasher) -> Self {
        Self {
            directory_path: value.directory_path.as_path(),
            custom_ignore_source: value.custom_ignore_source.as_deref(),
            respect_hidden: value.respect_hidden,
            respect_ignore: value.respect_ignore,
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
        let ignore_list = self.build_ignore_list()?;
        let errors = ArcVec::new();
        let paths = ArcVec::with_capacity(FILE_CAP_DEFAULT_GLOBAL);

        let self_ref = &self;
        let dir_path = self.directory_path.to_owned();
        let ignore_list_ref = &ignore_list;
        let errors_clone = errors.clone();
        let paths_clone = paths.clone();
        rayon::in_place_scope(move |scope| {
            self_ref.im_the_carrot_king(scope, dir_path, ignore_list_ref, errors_clone, paths_clone)
        });

        // SAFETY: rayon::in_place_scope is a blocking operation, so by this
        // point all clones of `errors` and `paths` will have been dropped and
        // the strong reference count will be 1 for both variables.
        let errors = unsafe { errors.into_inner() };
        let paths = unsafe { paths.into_inner() };
        // If any errors were found, we only worry about propagating the first.
        match errors.into_iter().next() {
            None => Ok(paths),
            Some(e) => Err(e),
        }
    }

    /// Sometimes you just need to eat a carrot. Fuck I forgot to get carrots when I went to Aldi's.
    ///
    /// For directory `dir_path`, sends all file paths into `paths`, spawns a new parallel instance for
    /// all directories, and sends any errors encountered into `errors`. Newly spawned instances will
    /// terminate immediately if `errors` contains any errors, but will finish working within their current
    /// directory if an error is pushed in some other worker during their execution.
    fn im_the_carrot_king(
        &'a self,
        scope: &rayon::Scope<'a>,
        dir_path: Utf8PathBuf,
        ignore_list: &'a GlobSet,
        errors: ArcVec<Error>,
        paths: ArcVec<Utf8PathBuf>,
    ) {
        // Kill procedure early if an error has already been encountered.
        // Only check once to avoid excessive lock contention.
        if !errors.lock().is_empty() {
            return;
        }
        let entries = unwrap_or_push_error_and_return!(dir_path.read_dir_utf8(), errors);
        // Every time we need to operate on `paths` we have to lock it's mutex,
        // so it's best to keep all our paths in a local vec and just append
        // all of them in bulk after all entries have been scanned.
        let mut paths_local = Vec::with_capacity(FILE_CAP_DEFAULT_LOCAL);
        for entry in entries {
            let entry = unwrap_or_push_error_and_return!(entry, errors);
            // Skip operating on an entry as soon as we have enough information to do so.
            if (self.respect_hidden && entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX))
                || (self.respect_ignore && ignore_list.is_match(entry.path().as_std_path()))
                || entry.file_name() == HASHFILE
            {
                continue;
            }
            let metadata = unwrap_or_push_error_and_return!(entry.metadata(), errors);
            // We prefer to use `Utf8PathBuf` because `Utf8DirEntry` contains things we don't have
            // any use for, and it is absolutely massive on windows platforms.
            let path = entry.into_path();
            if metadata.is_file() {
                if metadata.len() > 0 {
                    paths_local.push(path);
                }
            } else if metadata.is_dir() {
                let errors_clone = errors.clone();
                let paths_clone = paths.clone();
                scope.spawn(move |new_scope| {
                    self.im_the_carrot_king(new_scope, path, ignore_list, errors_clone, paths_clone)
                });
            }
        }
        paths.lock().extend(paths_local);
    }

    /// Constructs a [`GlobSet`] for ignoring files/directories using the provided ignore file.
    fn build_ignore_list(&self) -> Result<GlobSet, Error> {
        let mut gs_builder = GlobSet::builder();
        if self.respect_ignore {
            let ignore_file = self.custom_ignore_source.unwrap_or(IGNOREFILE.into());
            let ignore_path = self.directory_path.join(ignore_file);
            fs::read_to_string(ignore_path)?
                .trim()
                .lines()
                .map(str::trim)
                .filter(|s| s.chars().next().is_some_and(|s| s != IGNOREFILE_COMMENT))
                .try_for_each(|glob| {
                    let pattern = Glob::new(glob)?;
                    gs_builder.add(pattern);
                    Ok::<(), globset::Error>(())
                })?;
        }
        let gs = gs_builder.build()?;
        Ok(gs)
    }
}
