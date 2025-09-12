use camino::{Utf8Path, Utf8PathBuf};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::{fs, io};

const CAPACITY_PATHS: usize = 1 << 20;
const CAPACITY_TMP_PATHS: usize = 1 << 8;
const HIDDEN_ENTRY_PREFIX: char = '.';
const IGNOREFILE_COMMENT: char = '#';

#[derive(bon::Builder)]
pub struct FileFinder<'a> {
    directory_path: &'a Utf8Path,

    custom_ignore_source: Option<&'a str>,

    follow_symlinks: bool,

    respect_hidden: bool,

    respect_ignore: bool,
}

impl<'a> FileFinder<'a> {
    pub fn run(self) -> io::Result<Vec<Utf8PathBuf>> {
        let ignore_file = self.custom_ignore_source.unwrap_or(crate::IGNOREFILE);
        let ignore_list = Self::build_ignore_list(self.directory_path, ignore_file)?;

        todo!()
    }

    /// Constructs a [`GlobSet`] for ignoring files/directories using the provided ignore file.
    fn build_ignore_list(dir_path: &Utf8Path, ignore_file: &str) -> io::Result<GlobSet> {
        let mut builder = GlobSet::builder();
        let ignore_path = dir_path.join(ignore_file);
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
}
