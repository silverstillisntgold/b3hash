use crate::IOResult;
use crate::types::FileDescriptor;
use camino::Utf8Path;
use rayon::prelude::*;

const HIDDEN_ENTRY_PREFIX: char = '.';
const STARTING_CAP: usize = 1 << 8;

/// Builds a `Vec` containing the paths of all visible files beneath `dir_path`.
///
/// The ordering of these paths is non-deterministic.
#[inline(never)]
pub fn get_files(dir_path: &Utf8Path) -> IOResult<Vec<FileDescriptor>> {
    // Need to build a Vec first so we can use `into_par_iter`.
    let root_entries = dir_path.read_dir_utf8()?.collect::<IOResult<Vec<_>>>()?;
    root_entries
        .into_par_iter()
        .filter(|e| !e.file_name().starts_with(HIDDEN_ENTRY_PREFIX))
        .try_fold(
            || Vec::with_capacity(STARTING_CAP),
            |mut acc, entry| {
                let md = entry.metadata()?;
                let file_type = md.file_type();
                let path = entry.into_path();
                if file_type.is_file() {
                    let size = md.len();
                    if size > 0 {
                        acc.push(FileDescriptor { path, size });
                    }
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
