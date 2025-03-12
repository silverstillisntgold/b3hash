use crate::{types::FileDescriptor, IOResult};
use camino::{Utf8Path, Utf8PathBuf};

const HIDDEN_ENTRY_PREFIX: char = '.';
/// 20 MiB pre-allocation.
const STARTING_CAP_FILES: usize = 1 << 19;
/// 1 MiB pre-allocation.
const STARTING_CAP_FOLDERS: usize = 1 << 15;

/// Builds a `Vec` containing the paths of all visible
/// files beneath `dir_path`.
///
/// The ordering of these paths is non-deterministic
/// (we are at the mercy of the OS).
#[inline(never)]
pub fn get_files(dir_path: &Utf8Path) -> IOResult<Vec<FileDescriptor>> {
    let mut files = Vec::with_capacity(STARTING_CAP_FILES);
    let mut folders = Vec::with_capacity(STARTING_CAP_FOLDERS);
    // Seed with root directory.
    folders.push(dir_path.to_path_buf());
    while let Some(cur_folder) = folders.pop() {
        push_entries(cur_folder.as_path(), &mut files, &mut folders)?;
    }
    Ok(files)
}

/// Pushes all files and folders beneath `dir_path` into
/// their respective `Vec`.
///
/// Any entry that is marked as hidden is completely skipped.
/// Visible files within hidden folders are just as hidden as files
/// that themselves are hidden. Other entry types are ignored.
fn push_entries(
    dir_path: &Utf8Path,
    files: &mut Vec<FileDescriptor>,
    folders: &mut Vec<Utf8PathBuf>,
) -> IOResult<()> {
    for entry in dir_path.read_dir_utf8()? {
        let entry = entry?;
        if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) == false {
            // Cache metadata, since `Utf8PathBuf` doesn't store this information.
            let md = entry.metadata()?;
            // `Utf8PathBuf` is significantly smaller than `Utf8DirEntry`.
            let path = entry.into_path();
            if md.is_file() {
                let size = md.len();
                files.push(FileDescriptor { path, size });
            } else if md.is_dir() {
                folders.push(path);
            }
        }
    }
    Ok(())
}
