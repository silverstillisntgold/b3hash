use crate::fs::get_files;
use crate::types::HashedFile;
use crate::IOResult;
use blake3::{Hash, Hasher};
use camino::Utf8Path;
use rayon::prelude::*;
use std::io::{Error, ErrorKind};

const DELIM: char = ' ';
const NEWLINE: char = '\n';
const REPLACEMENT: char = '/';
const WINDOWS_MOMENT: char = '\\';

/// Builds a `Vec` by hashing all visible files beneath `dir_path`.
/// The returned `Vec` is always sorted by file path.
///
/// There are multiple to approach this. The most naive approach
/// (the first thing I tried lol) is to iterate sequentially over the
/// file list and hash each distinct file in parallel. But for small
/// files parallel hashing cost more than it pays out, and since directories
/// often contain many relatively small files, this isn't ideal.
/// Instead, we can "iterate" in parallel over our list of files,
/// and have each file be hashed sequentially using memory mapping.
/// Internally, memory mapping will allocate a small buffer instead of
/// mapping when the file is (roughly) too small to benefit from it.
///
/// But when hashing folders which contain many small files and a few
/// very large ones (like video game directories), it might be the case
/// that we chew threw all the small files near-instantly, but the last
/// few large files are then stuck chugging away. Since each file only
/// hashes on a single thread, this approach may be leaving performance
/// on the table. The issue is that blake3 is extremely fast even when
/// single-threaded. So fast, in fact, that my poor old SATA SSD is instantly
/// maxed out regardless of what directory I'm hashing. That being said,
/// I'm currently unable to properly test how nested parallelism would
/// perform in a scenario like this. I imagine servers with 50+ GiB/s read
/// speed would greatly benefit from being able to always fully utilize
/// however many threads they've given to b3hash to work with.
///
/// This approach would unfortunately introduce the problem of needing to store
/// a larger struct to also know the size of each file to conditionally
/// decide whether to hash serially or in parallel. The current solution
/// is very simple and very fast, but adding additional complexity
/// might be worth it if I could guarantee improved directory hashing
/// speed on directories with a mix of very large/small files. Even more
/// so if I could avoid performance regressions with directories almost
/// exclusively containing smaller files.
pub fn hash_files(dir_path: &str) -> IOResult<Vec<HashedFile>> {
    // One more than the actual length because we don't want
    // stripped file paths to start with a slash.
    // Both slash types are just ascii (a single byte in utf8),
    // so this still lands on a valid utf8 boundary.
    let prefix_len = dir_path.len() + 1;

    let mut file_list = get_files(dir_path.into())?;
    file_list.sort_unstable();

    file_list
        .into_par_iter()
        .map(|file_path| {
            // Using memory mapping is more-or-less mandatory here. If we
            // were to instead use regular update() we'd need to explicitly
            // load each file into memory and pass a reference to that buffer.
            // Since we're running all these file hashes in parallel, any
            // folder containing enough large files to exceed available RAM will
            // quickly do so, making the system extremely unresponsive.
            // Memory mapping uses cached/standby memory, which allows other
            // running programs that have explicitly allocated memory
            // to maintain priority.
            let mut hasher = Hasher::new();
            hasher.update_mmap(file_path.as_std_path())?;
            // SAFETY: Since all files are descendants of dir_path,
            // they all have dir_path as a prefix.
            let stripped_file_path = unsafe { file_path.as_str().get_unchecked(prefix_len..) };
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

/// TODO: docs
pub fn serialize_hashed_files(hashed_files: Vec<HashedFile>) -> Vec<u8> {
    /// 20MiB pre-allocation.
    const STARTING_CAP: usize = 20 * (1 << 20);
    hashed_files
        .into_iter()
        .fold(Vec::with_capacity(STARTING_CAP), |mut buf, file| {
            // Prefer to_hex() over to_string() because it avoids heap allocation.
            buf.extend(file.hash.to_hex().bytes());
            // The char constants used are represented as ascii values,
            // so forcing them into u8's and pushing them is fine.
            buf.push(DELIM as u8);
            buf.extend(file.path.bytes());
            buf.push(NEWLINE as u8);
            buf
        })
}

/// Simultaneously parses **and** validates file hashes from `old_data`,
///
/// Since each line contains both the file path relative to `dir_path`
/// and the hash for said file, upon successfully parsing each line we
/// can immedietely hash the associated file and compare hashes.
pub fn validate_data(dir_path: &str, old_data: Vec<u8>) -> IOResult<Option<Vec<String>>> {
    // Caller may actually see these paths when files fail validation
    // or errors are returned so we erase windows retardation if it exists.
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
    let failed_files = unsafe { String::from_utf8_unchecked(old_data) }
        .par_lines()
        .filter_map(|line| {
            // Each line first needs to be partitioned into it's two parts:
            // the hash itself and the file path for the file the hash
            // was derived from. The aforementioned file path only contains
            // it's path relative to `dir_path` (foreshadowing).
            match line.split_once(DELIM) {
                // We want the hash to be a literal Hash value instead of
                // the String representation of one, since Hash has a
                // specialized eq() that's much faster than the eq() of String.
                Some((hash, file_path)) => match Hash::from_hex(hash) {
                    Ok(old_hash) => {
                        // Since file paths have been stripped of their common prefix,
                        // which is always the relative path to their root directory,
                        // it needs to be re-added.
                        let path = Utf8Path::new(dir_path).join(file_path);
                        // Rust documentation recommends against using
                        // .exists(), so we don't.
                        match path.try_exists() {
                            Ok(true) => match Hasher::new().update_mmap(path.as_std_path()) {
                                Ok(hasher) => {
                                    let new_hash = hasher.finalize();
                                    match hash_eq(&old_hash, &new_hash) {
                                        true => None,
                                        false => Some(Ok(path.into_string())),
                                    }
                                }
                                // My assumption is that if .try_exists() suceeds then
                                // .update_mmap() should as well, but we still
                                // handle the case where it doesn't.
                                Err(e) => Some(Err(e)),
                            },
                            // No errors but file doesn't exist, so we add
                            // as one of the files that failed validation.
                            Ok(false) => Some(Ok(path.into_string())),
                            // Error'd while determining if file exists.
                            // Only scenarios where I actually think this might
                            // proc is is file/folder permission is denied.
                            Err(e) => Some(Err(e)),
                        }
                    }
                    // HexError needs to be explicitly converted to IOError.
                    Err(e) => Some(Err(Error::new(ErrorKind::InvalidData, e.to_string()))),
                },
                // Delimiter wasn't found on current line (how tf???)
                // so we cancel verification and propagate an error.
                None => Some(Err(Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "Failed to find delimiter '{}' while parsing line '{}'.",
                        DELIM, line
                    ),
                ))),
            }
        })
        .collect::<IOResult<Vec<_>>>()?;

    // The length of failed_files is the amount
    // of files that failed validation.
    match failed_files.len() {
        0 => Ok(None),
        _ => Ok(Some(failed_files)),
    }
}

#[inline(always)]
fn hash_eq(x: &Hash, y: &Hash) -> bool {
    if cfg!(target_arch = "x86_64") {
        // Always constant time on x64 platforms, and faster
        // than provided Hash::eq.
        x.as_bytes().eq(y.as_bytes())
    } else {
        // May not be constant time so defer to provided Hash::eq,
        // which is guaranteed to always be constant time.
        x.eq(y)
    }
}
