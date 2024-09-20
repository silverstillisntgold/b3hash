use crate::IOResult;
use camino::{Utf8Path, Utf8PathBuf};

/// Simultaneously store all visible files in LLVM 19 without reallocating.
const STARTING_CAP_FILES: usize = 1 << 18;
/// Simultaneously store all visible folders in LLVM 19 without reallocating.
const STARTING_CAP_FOLDERS: usize = 1 << 14;

/// Builds a `Vec` containing the relative paths of all visible
/// files that live beneath `dir_path`.
///
/// The ordering of these paths is non-deterministic
/// (we are at the mercy of the OS).
#[inline(never)]
pub fn get_files(dir_path: &Utf8Path) -> IOResult<Vec<Utf8PathBuf>> {
    let mut files = Vec::with_capacity(STARTING_CAP_FILES);
    let mut folders = Vec::with_capacity(STARTING_CAP_FOLDERS);
    // Seed first .pop() with root directory.
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
/// that themselves are hidden. Any other entry type is ignored.
#[inline]
fn push_entries(
    dir_path: &Utf8Path,
    files: &mut Vec<Utf8PathBuf>,
    folders: &mut Vec<Utf8PathBuf>,
) -> IOResult<()> {
    const HIDDEN_ENTRY_PREFIX: char = '.';
    for entry in dir_path.read_dir_utf8()? {
        let entry = entry?;
        // Only consider visible entries.
        if !entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
            // Retrieve type first, since Utf8PathBuf
            // doesn't store this information.
            let entry_type = entry.file_type()?;
            // Utf8PathBuf is significantly smaller than Utf8DirEntry.
            let entry = entry.into_path();
            if entry_type.is_file() {
                files.push(entry);
            } else if entry_type.is_dir() {
                folders.push(entry);
            }
        }
    }
    Ok(())
}
