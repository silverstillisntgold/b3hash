mod fs;
mod types;
mod util;

use blake3::Hasher;
use camino::Utf8Path;
use types::HashedDirectory;
use util::*;

/// Convenience type for `std::io::Result` with more explicit name.
pub type IOResult<Type> = std::io::Result<Type>;

/// TODO: docs
pub const HASH_RESULTS_FILENAME: &str = ".b3hash_v1";

/// TODO: docs
#[inline(never)]
pub fn hash_directory(dir_path: &str) -> IOResult<HashedDirectory> {
    // It is absolutely critical that the returned Vec always
    // returns the same ordering of file hashes, given the same root
    // directory. Otherwise, the overall directory hash will be random.
    // In our case this is solved by having hash_files_vec()
    // internally sort the Vec by file path before returning.
    let hashed_files = hash_files(dir_path)?;
    let mut total_bytes_hashed = 0;
    let mut hasher = Hasher::new();

    // It's slightly faster to fold the bytes of each file's hash and name
    // into a Vec<u8>, then hash that, because the hasher is able to use
    // vector instructions more consistently on larger [u8]'s.
    // But the difference is insignificant for small directories,
    // and for large directories the time spent here is miniscule
    // compared to overall directory file hashing, so this simple
    // and in-place implementation is prefered.
    for file in &hashed_files {
        hasher.update(file.hash.as_bytes());
        hasher.update(file.path.as_bytes());
        total_bytes_hashed += file.size;
    }

    Ok(HashedDirectory {
        dir_name: Utf8Path::new(dir_path)
            .file_name()
            .unwrap_or(dir_path)
            .to_string(),
        files: hashed_files,
        hash: hasher.finalize(),
        size: total_bytes_hashed,
    })
}

/// TODO: docs
#[inline(never)]
pub fn create_hashfile(dir_path: &str) -> IOResult<()> {
    let hashfile_path = Utf8Path::new(".").join(HASH_RESULTS_FILENAME);
    let hashed_files = hash_files(dir_path)?;
    let data = serialize_hashed_files(hashed_files);
    std::fs::write(hashfile_path, data)?;
    Ok(())
}

/// TODO: docs
#[inline(never)]
pub fn validate_hashfile(dir_path: &str) -> IOResult<Option<Vec<String>>> {
    let hashfile_path = Utf8Path::new(".").join(HASH_RESULTS_FILENAME);
    let data = std::fs::read(hashfile_path)?;
    validate_data(dir_path, data)
}

/// Alias for `hash_directory`, but with `num_threads` number
/// of threads to be used in the rayon threadpool.
pub fn hash_directory_with_threads(
    dir_path: &str,
    num_threads: usize,
) -> IOResult<HashedDirectory> {
    with_threads(num_threads, || hash_directory(dir_path))
}

/// Alias for `create_hashfile`, but with `num_threads` number
/// of threads to be used in the rayon threadpool.
pub fn create_hashfile_with_threads(dir_path: &str, num_threads: usize) -> IOResult<()> {
    with_threads(num_threads, || create_hashfile(dir_path))
}

/// Alias for `validate_hashfile`, but with `num_threads` number
/// of threads to be used in the rayon threadpool.
pub fn validate_hashfile_with_threads(
    dir_path: &str,
    num_threads: usize,
) -> IOResult<Option<Vec<String>>> {
    with_threads(num_threads, || validate_hashfile(dir_path))
}

/// Convenience method for spawning new rayon threadpool with
/// a set number of threads.
fn with_threads<F, R>(num_threads: usize, func: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        // Is this actually the case or should the error be propagated?
        .expect("BUG: Initializing unique threadpools should never fail.")
        .install(func)
}
