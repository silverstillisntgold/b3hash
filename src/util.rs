use crate::IOResult;
use crate::fs::*;
use crate::types::HashedFile;
use blake3::{Hash, Hasher};
use camino::Utf8Path;
use rayon::prelude::*;
use std::fs::File;
use std::io::{Error, ErrorKind};

const DELIM: char = ' ';
const NEWLINE: char = '\n';
const REPLACEMENT: char = '/';
const WINDOWS_MOMENT: char = '\\';

/// Convenience method for calling a function inside one-time
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

/// Builds a `Vec` by hashing all visible files beneath `dir_path`.
/// The returned `Vec` is always sorted by file path.
///
/// There are a few ways to approach this. The most naive approach
/// (the first thing I tried lol) is to iterate sequentially over the
/// file list and hash each distinct file in parallel. But for small
/// files parallel hashing costs more than it pays out, and since directories
/// often contain many relatively small files, this isn't ideal.
/// Instead, we can "iterate" in parallel over the whole list of files
/// and have each file be hashed either sequentially or in parallel,
/// depending on it's size. Both of these processes use memory mapping.
/// Internally, memory mapping will allocate a small buffer instead of
/// mapping when the file is approximately too small to benefit from it.
pub fn hash_files(dir_path: &str) -> IOResult<Vec<HashedFile>> {
    // One more than the actual length because we don't want
    // stripped file paths to start with a slash.
    // Both slash types are just ascii (a single byte in utf8),
    // so this still lands on a valid utf8 boundary.
    let prefix_len = dir_path.len() + 1;

    let start = std::time::Instant::now();
    let mut file_list = get_files(dir_path.into())?;
    let delta = std::time::Instant::now()
        .duration_since(start)
        .as_secs_f64();
    println!("Time to collect files: {:.2} seconds", delta);
    // It's more effective for sorting to be done here,
    // since `Utf8PathBuf` is effectively just a `String`,
    // and is faster to sort than `HashedFile`.
    file_list.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));

    file_list
        .into_par_iter()
        .map(|file| {
            let mut hasher = Hasher::new();
            let reader = File::open(file.as_std_path())?;
            // Calls to `Hasher::update_reader` internally buffer 64KiB of
            // data, so we don't need to worry about doing that manually.
            hasher.update_reader(reader)?;
            // SAFETY: Since all files are descendants of dir_path,
            // they all have dir_path as a prefix.
            let stripped_file_path = unsafe { file.as_str().get_unchecked(prefix_len..) };
            Ok(HashedFile {
                hash: hasher.finalize(),
                path: oi_vei(stripped_file_path),
                size: hasher.count(),
            })
        })
        .collect()
}

/// Windows always has to be so funny and unique >:(
#[inline]
fn oi_vei(s: &str) -> String {
    if cfg!(windows) {
        s.chars()
            .map(|c| match c == WINDOWS_MOMENT {
                false => c,
                true => REPLACEMENT,
            })
            .collect()
    } else {
        s.to_string()
    }
}

/// Collapses data from `hashed_files` into a `Vec` of bytes. This data
/// represents a newline-deliniated `String` containing pairs of hashes
/// and the file paths from which they were derived.
///
/// It's possible to parallelize this operation, using `rayon::flat_map`,
/// but doing so regresses performance significantly.
pub fn serialize_hashed_files(hashed_files: Vec<HashedFile>) -> Vec<u8> {
    /// 32MiB pre-allocation.
    const STARTING_CAP: usize = 1 << 25;
    hashed_files
        .into_iter()
        .fold(Vec::with_capacity(STARTING_CAP), |mut buf, file| {
            // Prefer `to_hex` over `to_string` because it avoids heap allocation.
            buf.extend_from_slice(file.hash.to_hex().as_bytes());
            // The char constants used are represented as ascii values,
            // so forcing them into u8's and pushing them is fine.
            buf.push(DELIM as u8);
            buf.extend_from_slice(file.path.as_bytes());
            buf.push(NEWLINE as u8);
            buf
        })
}

/// Simultaneously parses **and** validates file hashes from `old_data`,
/// returning a list of file paths which failed validation, or returning
/// early with an IO error.
///
/// Since each line contains both the file path relative to `dir_path`
/// and the hash for said file, upon successfully parsing each line we
/// can immediately hash the associated file and compare hashes.
pub fn validate_data(dir_path: &str, old_data: Vec<u8>) -> IOResult<Vec<String>> {
    // Caller may actually see these paths when files fail validation or errors
    // are returned, so we override windows retardation if it exists.
    let dir_path_frfr = oi_vei(dir_path);
    let dir_path = dir_path_frfr.as_str();

    // We're building a Vec<String> containing the names of files
    // which either are not present in our new data or whose
    // new Hash does not match the old Hash.
    //
    // SAFETY: Old hashfile data should always be valid utf8
    // because we serialize into valid utf8. Users changing
    // hashfile contents or not verifying the hashfile itself
    // before using it to verify a directory is a user error.
    unsafe { String::from_utf8_unchecked(old_data) }
        .par_lines()
        .filter_map(|line| {
            // Each line first needs to be partitioned into it's two parts:
            // the hash itself and the file path the hash was derived from.
            match line.split_once(DELIM) {
                // We want the hash to be a literal Hash value instead of
                // the String representation of one, since Hash has a
                // specialized eq() that's much faster than the eq() of String.
                Some((hash, file_path)) => match Hash::from_hex(hash) {
                    Ok(old_hash) => {
                        // Since file paths are always stripped of their common prefix,
                        // which is always the relative path to their root directory,
                        // it needs to be re-added.
                        let path = Utf8Path::new(dir_path).join(file_path);
                        match path.try_exists() {
                            Ok(true) => {
                                let file = File::open(path.as_std_path()).unwrap();
                                match Hasher::new().update_reader(file) {
                                    Ok(hasher) => {
                                        let new_hash = hasher.finalize();
                                        match hash_eq(&old_hash, &new_hash) {
                                            true => None,
                                            // File exists but it's hash is incorrect:
                                            // IT'S CORRUPTED OH NO.
                                            false => Some(Ok(path.into_string())),
                                        }
                                    }
                                    // I have zero clue when this would ever trigger.
                                    Err(e) => Some(Err(e)),
                                }
                            }
                            // No errors but file doesn't exist, so we add
                            // as one of the files that failed validation.
                            Ok(false) => Some(Ok(path.into_string())),
                            // Error'd while determining if file exists.
                            // Only scenarios where I actually think this might
                            // proc is if file/folder permission is denied.
                            Err(e) => Some(Err(e)),
                        }
                    }
                    // HexError needs to be explicitly converted to IOError.
                    Err(e) => Some(Err(Error::new(ErrorKind::InvalidData, e))),
                },
                // Delimiter wasn't found on current line (how tf???)
                // so we cancel verification and propagate an error.
                None => Some(Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "Failed to find delimiter '{}' while parsing line '{}'.",
                        DELIM, line
                    ),
                ))),
            }
        })
        .collect()
}

#[inline]
fn hash_eq(x: &Hash, y: &Hash) -> bool {
    if cfg!(target_arch = "x86_64") || cfg!(target_arch = "x86") {
        // Always constant time on x86 platforms, and faster
        // than provided `Hash::eq`.
        x.as_bytes().eq(y.as_bytes())
    } else {
        // May not be constant time so defer to provided `Hash::eq`,
        // which is guaranteed to be constant time.
        x.eq(y)
    }
}
