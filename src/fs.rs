use crate::IOResult;
use crate::types::FileDescriptor;
use camino::Utf8PathBuf;

const HIDDEN_ENTRY_PREFIX: char = '.';
/// 20 MiB pre-allocation.
const STARTING_CAP_FILES: usize = 1 << 19;
/// 1 MiB pre-allocation.
const STARTING_CAP_FOLDERS: usize = 1 << 15;

/// Builds a `Vec` containing the paths of all visible files beneath `dir_path`.
///
/// The ordering of these paths is non-deterministic (we are at the mercy of the OS).
#[inline(never)]
pub fn get_files(dir_path: &str) -> IOResult<Vec<FileDescriptor>> {
    let mut files = Vec::with_capacity(STARTING_CAP_FILES * 10);
    let mut folders = Vec::with_capacity(STARTING_CAP_FOLDERS * 10);
    // Seed with root directory.
    folders.push(Utf8PathBuf::from(dir_path));
    while let Some(cur_folder) = folders.pop() {
        for entry in cur_folder.read_dir_utf8()? {
            let entry = entry?;
            // Visible files within hidden folders are just as hidden as files
            // that themselves are hidden. Other entry types are ignored.
            if !entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
                // Cache metadata, since `Utf8PathBuf` doesn't store this information.
                let md = entry.metadata()?;
                // `Utf8PathBuf` is smaller than `Utf8DirEntry`.
                let path = entry.into_path();
                if md.is_file() {
                    let size = md.len();
                    if size > 0 {
                        files.push(FileDescriptor { path, size });
                    }
                } else if md.is_dir() {
                    folders.push(path);
                }
            }
        }
    }
    files.shrink_to_fit();
    Ok(files)
}
