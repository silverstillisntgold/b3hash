/*!
# B3Hash

A crate for creating/validating directory tree hashfiles.
*/

mod arcvec;
mod fs;
mod types;
mod util;

use blake3::Hasher;
use camino::{Utf8Path, Utf8PathBuf};
use std::io;
use types::HashedDirectory;
use util::*;

/// TODO: docs
pub const HASH_RESULTS_FILENAME: &str = ".b3hash_v0";

/// TODO: docs
pub fn hash_directory(dir_path: &str) -> io::Result<HashedDirectory> {
    // It is absolutely critical that the returned Vec always
    // returns the same ordering of file hashes, given the same root
    // directory. Otherwise, the overall directory hash will be random.
    // In our case this is solved by having hash_files() internally
    // sort the Vec by file path before returning.
    let hashed_files = hash_files(dir_path)?;
    let mut total_bytes_hashed = 0;
    let mut hasher = Hasher::new();

    // It's slightly faster to fold the bytes of each file's hash
    // into a Vec<u8>, then hash that, because the hasher is able to use
    // vector instructions more consistently on larger [u8]'s.
    // But the difference is insignificant for small directories,
    // and for large directories the time spent here is miniscule
    // compared to overall directory file hashing, so this simple
    // and in-place implementation is prefered.
    for file in &hashed_files {
        hasher.update(file.hash.as_bytes());
        total_bytes_hashed += file.size;
    }

    Ok(HashedDirectory {
        name: Utf8Path::new(dir_path)
            .file_name()
            .unwrap_or(dir_path)
            .to_owned(),
        files: hashed_files,
        hash: hasher.finalize(),
        size: total_bytes_hashed,
    })
}

/// TODO: docs
pub fn create_hashfile(dir_path: &str) -> io::Result<()> {
    let hashfile_path = Utf8Path::new(dir_path).join(HASH_RESULTS_FILENAME);
    let hashed_files = hash_files(dir_path)?;
    let data = serialize_hashed_files(hashed_files);
    std::fs::write(hashfile_path, data)?;
    Ok(())
}

/// TODO: docs
pub fn validate_hashfile(dir_path: &str) -> io::Result<Option<Vec<String>>> {
    let hashfile_path = Utf8Path::new(dir_path).join(HASH_RESULTS_FILENAME);
    let data = std::fs::read(hashfile_path)?;
    let failed_files = validate_data(dir_path, data)?;
    // The length of failed_files is the amount
    // of files that failed validation.
    Ok(match failed_files.len() {
        0 => None,
        _ => Some(failed_files),
    })
}

/// TODO: docs
pub fn validate_hashfile_v2(dir_path: &str) -> io::Result<Option<Vec<Utf8PathBuf>>> {
    let hashfile_path = Utf8Path::new(dir_path).join(HASH_RESULTS_FILENAME);
    let data = std::fs::read(hashfile_path)?;
    _ = data;
    let file_list = vec![];
    validate_files(file_list)
}

/// TODO: docs
pub fn validate_files(file_list: Vec<Utf8PathBuf>) -> io::Result<Option<Vec<Utf8PathBuf>>> {
    let _ = file_list;
    todo!()
}

/// Convenience function for calling a function inside one-time
/// usage rayon threadpool with a custom number of threads.
pub fn with_threads<F, R>(num_threads: usize, func: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .expect("initializing unique threadpools should never fail")
        .install(func)
}
