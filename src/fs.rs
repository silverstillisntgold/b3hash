use crate::IOResult;
use crate::types::FileDescriptor;
use camino::Utf8PathBuf;
use rayon::prelude::*;

const HIDDEN_ENTRY_PREFIX: char = '.';
/// 20 MiB pre-allocation.
const STARTING_CAP_FILES: usize = 1 << 19;
/// 1 MiB pre-allocation.
const STARTING_CAP_FOLDERS: usize = 1 << 15;

#[allow(unused)]
#[inline(never)]
pub fn get_files_v3(dir_path: &str) -> IOResult<Vec<FileDescriptor>> {
    let path = Utf8PathBuf::from(dir_path);
    let root_entries = path.read_dir_utf8()?.collect::<IOResult<Vec<_>>>()?;

    let tmp = root_entries
        .into_par_iter()
        .filter(|e| !e.file_name().starts_with(HIDDEN_ENTRY_PREFIX))
        .filter_map(|e| match e.metadata() {
            Ok(md) => {
                let path = e.into_path();
                if md.file_type().is_file() {
                    let size = md.len();
                    if size > 0 {
                        Some(Ok(vec![FileDescriptor { path, size }]))
                    } else {
                        None
                    }
                } else if md.file_type().is_dir() {
                    match get_files_v3(path.as_str()) {
                        Ok(r) => Some(Ok(r)),
                        Err(e) => Some(Err(e)),
                    }
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        })
        .collect::<IOResult<Vec<_>>>()?;

    Ok(tmp.into_iter().flatten().collect())
}

#[allow(unused)]
#[inline(never)]
pub fn get_files_v2(dir_path: &str) -> IOResult<Vec<FileDescriptor>> {
    use std::sync::{Arc, Mutex};
    let files = Arc::new(Mutex::new(Vec::with_capacity(STARTING_CAP_FILES)));

    fn read_directory(
        files: Arc<Mutex<Vec<FileDescriptor>>>,
        path: Utf8PathBuf,
        s: &rayon::Scope<'_>,
    ) -> IOResult<()> {
        for entry in path.read_dir_utf8()? {
            let entry = entry?;
            if !entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
                let md = entry.metadata()?;
                let path = entry.into_path();
                if md.file_type().is_file() {
                    let size = md.len();
                    if size > 0 {
                        let file = FileDescriptor { path, size };
                        files.lock().unwrap().push(file);
                    }
                } else if md.file_type().is_dir() {
                    let files_copy = files.clone();
                    s.spawn(move |s1| read_directory(files_copy, path, s1).unwrap());
                }
            }
        }
        Ok(())
    }

    let path = Utf8PathBuf::from(dir_path);
    let files_copy = files.clone();
    rayon::scope(move |s| s.spawn(move |s1| read_directory(files_copy, path, s1).unwrap()));
    // Panics are unlikely here.
    let files = Arc::into_inner(files).unwrap().into_inner().unwrap();
    Ok(files)
}

/// Builds a `Vec` containing the paths of all visible files beneath `dir_path`.
///
/// The ordering of these paths is non-deterministic (we are at the mercy of the OS).
#[allow(unused)]
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
