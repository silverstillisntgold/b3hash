use crate::{DirectoryHasher, Error, HASHFILE, IGNOREFILE};
use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::Sender;
use globset::{Glob, GlobSet};
use std::fs;

const HIDDEN_ENTRY_PREFIX: char = '.';
const IGNOREFILE_COMMENT: char = '#';

pub struct FileFinder<'a> {
    directory_path: &'a Utf8Path,
    custom_ignore_source: Option<&'a str>,
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

macro_rules! unwrap_or_push_error_and_return {
    ($expr: expr, $err_chan_desu: ident) => {
        match $expr {
            Ok(value) => value,
            Err(e) => {
                // SAFETY: Calls to `Sender::try_send` will only return an error if
                // the channel being sent into is full or disconnected. The structure
                // of the code guarantees that receivers will always be alive longer
                // than senders, so being disconnected is impossible. And the channel's
                // are both unbounded, so they can never be full.
                unsafe {
                    $err_chan_desu.try_send(e.into()).unwrap_unchecked();
                }
                return;
            }
        }
    };
}

impl<'a> FileFinder<'a> {
    #[inline(never)]
    pub fn find(self) -> Result<Vec<Utf8PathBuf>, Error> {
        let ignore_list = self.build_ignore_list()?;
        let (error_s, error_r) = crossbeam_channel::unbounded::<Error>();
        let (path_s, path_r) = crossbeam_channel::unbounded::<Utf8PathBuf>();

        let dir_path = self.directory_path.to_owned();
        let ignore_list_ref = &ignore_list;
        let self_ref = &self;
        rayon::in_place_scope(move |scope| {
            self_ref.im_the_carrot_king(dir_path, scope, ignore_list_ref, error_s, path_s)
        });

        // No need to use blocking operations because both senders will have been dropped by this point.
        match error_r.try_recv() {
            // If the channel is empty then we have no errors, which is our success case.
            Err(_) => Ok(path_r.try_iter().collect()),
            // If the channel isn't empty then we have an error that needs to be propagated.
            Ok(e) => Err(e),
        }
    }

    /// Sometimes you just need to eat a carrot. Fuck I forgot to get carrots when I went to Aldi's.
    ///
    /// For directory `dir_path`, sends all file paths into `path_s`, spawns a new parallel instance for
    /// all directories, and sends any errors encountered into `error_s`. Newly spawned instances will
    /// terminate immediately if `error_s` contains any errors, but will finish working within their current
    /// directory if an error is pushed in some other worker during their execution.
    fn im_the_carrot_king(
        &'a self,
        dir_path: Utf8PathBuf,
        scope: &rayon::Scope<'a>,
        ignore_list: &'a GlobSet,
        error_s: Sender<Error>,
        path_s: Sender<Utf8PathBuf>,
    ) {
        // Kill procedure early if an error has already been encountered.
        if !error_s.is_empty() {
            return;
        }
        let entries = unwrap_or_push_error_and_return!(dir_path.read_dir_utf8(), error_s);
        for entry in entries {
            let entry = unwrap_or_push_error_and_return!(entry, error_s);
            // Skip operating on an entry as soon as we have enough information to do so.
            if (self.respect_hidden && entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX))
                || (self.respect_ignore && ignore_list.is_match(entry.path().as_std_path()))
                || entry.file_name() == HASHFILE
            {
                continue;
            }
            let metadata = unwrap_or_push_error_and_return!(entry.metadata(), error_s);
            // We prefer to use `Utf8PathBuf` because `Utf8DirEntry` contains things we don't have
            // any use for, and it is absolutely massive on windows platforms.
            let path = entry.into_path();
            if metadata.is_file() {
                if metadata.len() > 0 {
                    // SAFETY: Safe for the same reason the error macro is safe.
                    unsafe {
                        path_s.try_send(path).unwrap_unchecked();
                    }
                }
            } else if metadata.is_dir() {
                let error_s_clone = error_s.clone();
                let path_s_clone = path_s.clone();
                scope.spawn(move |new_scope| {
                    self.im_the_carrot_king(
                        path,
                        new_scope,
                        ignore_list,
                        error_s_clone,
                        path_s_clone,
                    )
                });
            }
        }
    }

    /// Constructs a [`GlobSet`] for ignoring files/directories using the provided ignore file.
    fn build_ignore_list(&self) -> Result<GlobSet, Error> {
        let mut builder = GlobSet::builder();
        if self.respect_ignore {
            let ignore_file = self.custom_ignore_source.unwrap_or(IGNOREFILE);
            let ignore_path = self.directory_path.join(ignore_file);
            fs::read_to_string(ignore_path)?
                .trim()
                .lines()
                .map(str::trim)
                .filter(|s| s.chars().next().is_some_and(|s| s != IGNOREFILE_COMMENT))
                .try_for_each(|glob| {
                    let pat = Glob::new(glob)?;
                    builder.add(pat);
                    Ok::<_, globset::Error>(())
                })?;
        }
        let gs = builder.build()?;
        Ok(gs)
    }
}
