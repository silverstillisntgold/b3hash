use crate::IOResult;
use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;

const HIDDEN_ENTRY_PREFIX: char = '.';
const CAPACITY: usize = 1 << 7;

/// Build a `Vec` containing the paths of all visible files within `dir_path`.
///
/// The ordering of these paths is non-deterministic.
#[inline(never)]
pub fn get_files(dir_path: &Utf8Path) -> IOResult<Vec<Utf8PathBuf>> {
    // Need to build a `Vec` first so we can use `into_par_iter`.
    let root_entries = dir_path.read_dir_utf8()?.collect::<IOResult<Vec<_>>>()?;
    root_entries
        .into_par_iter()
        .filter(|e| !e.file_name().starts_with(HIDDEN_ENTRY_PREFIX))
        .try_fold(
            || Vec::with_capacity(CAPACITY),
            |mut acc, entry| {
                let metadata = entry.metadata()?;
                let file_type = metadata.file_type();
                let path = entry.into_path();
                if file_type.is_file() && metadata.len() > 0 {
                    acc.push(path);
                } else if file_type.is_dir() {
                    let tmp = get_files(path.as_path())?;
                    acc.extend(tmp);
                }
                Ok(acc)
            },
        )
        .try_reduce(
            || Vec::new(),
            |mut a, b| {
                a.extend(b);
                Ok(a)
            },
        )
}
