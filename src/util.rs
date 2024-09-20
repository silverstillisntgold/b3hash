use crate::fs::get_files;
use crate::types::HashedFile;
use crate::IOResult;
use blake3::{Hash, Hasher};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

const DELIM: char = ' ';
const NEWLINE: char = '\n';
const REPLACEMENT: char = '/';
const WINDOWS_MOMENT: char = '\\';

/// Specialization of `hash_files_core` for `HashMap`.
#[inline(never)]
pub fn hash_files_map(dir_path: &str) -> IOResult<HashMap<String, Hash>> {
    hash_files_core(dir_path)
}

/// Specialization of `hash_files_core` for `Vec`.
///
/// The returned `Vec` is sorted by the file path of each item.
#[inline(never)]
pub fn hash_files_vec(dir_path: &str) -> IOResult<Vec<HashedFile>> {
    let mut hashed_files: Vec<HashedFile> = hash_files_core(dir_path)?;
    hashed_files.sort_unstable_by(|a, b| a.cmp(b));
    Ok(hashed_files)
}

/// Builds a collection by hashing all visible files beneath
/// `dir_path` and turning the mapped result into the desired type.
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
/// This approach would unfortunetly introduce the problem of needing to store
/// a larger struct to also know the size of each file to conditionally
/// decide whether to hash serially or in parallel. The current solution
/// is very simple and very fast, but adding additional complexity
/// would 100% be worth it if I could guarantee improved directory hashing
/// speed on directories with a mix of very large/small files. Even more
/// so if I could avoid performance regressions with directories almost
/// exclusively containing smaller files.
#[inline(always)]
fn hash_files_core<C, T>(dir_path: &str) -> IOResult<C>
where
    C: FromParallelIterator<T>,
    T: From<HashedFile> + Send,
{
    // One more than the actual length because we don't want
    // stripped file paths to start with a slash.
    // Both slash types are just ascii (a single byte in utf8),
    // so this still lands on a valid utf8 boundary.
    let prefix_len = dir_path.len() + 1;

    get_files(dir_path.into())?
        .into_par_iter()
        .map(|file_path| {
            let mut hasher = Hasher::new();
            // Using memory mapping is more-or-less mandatory here. If we
            // were to instead use regular update() we'd need to explicitly
            // load each file into memory and pass a reference to that buffer.
            // Since we're running all these file hashes in parallel, any
            // folder containing enough large files to exceed available RAM will
            // quickly do so, making the system extremely unresponsive.
            // Memory mapping uses cached/standby memory, which allows other
            // running programs that have explicitly allocated memory
            // to maintain priority.
            hasher.update_mmap(file_path.as_str())?;
            // SAFETY: Since all files are descendants of dir_path,
            // they all have dir_path as a prefix.
            let stripped_file_path = unsafe { file_path.as_str().get_unchecked(prefix_len..) };
            Ok(T::from(HashedFile {
                hash: hasher.finalize(),
                path: oi_vei(stripped_file_path),
                size: hasher.count(),
            }))
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
            buf.extend_from_slice(file.hash.to_hex().as_bytes());
            // The char constants used are represented as ascii values,
            // so forcing them into u8's and pushing them is fine.
            buf.push(DELIM as u8);
            buf.extend_from_slice(file.path.as_bytes());
            buf.push(NEWLINE as u8);
            buf
        })
}

/// TODO: docs
pub fn parse_old_data(data: Vec<u8>) -> IOResult<Vec<(String, Hash)>> {
    // SAFETY: Old hashfile data should always be valid utf8
    // because we serialize into valid utf8. Users changing
    // hashfile contents or not veryifying the hashfile itself
    // before trying to verify a directory is a user error so
    // we don't waste time checking for it.
    // Despite that, any imperfections in the resulting String
    // will either propagate an error during parsing or an invalid
    // file result during verification, neither of which
    // are all that disastrous.
    // TODO: Document that I've chosen to take this approach in
    // user-facing code.
    unsafe { String::from_utf8_unchecked(data) }
        .par_lines()
        .map(|s| {
            let (hash, name) = s.split_once(DELIM).ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "Failed to find delimiter '{}' while parsing line '{}'.",
                        DELIM, s
                    ),
                )
            })?;
            // We want the hash portion of the returned Vec tuple to be
            // a literal Hash value instead of the String representation
            // of one, since Hash has a specialized eq() that's much
            // faster than the eq() of String.
            let hash = Hash::from_hex(hash)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
            Ok((name.to_string(), hash))
        })
        .collect()
}

/// Compares `old` and `new` and **potentially** returns a `Vec`
/// containing the paths of files which failed validation (they either
/// weren't present in `new` or their Hash was incorrect).
///
/// Returns `None` when there are no files that failed validation.
pub fn validate_data(old: Vec<(String, Hash)>, new: HashMap<String, Hash>) -> Option<Vec<String>> {
    // We're building a Vec<String> containing the names of files
    // which either are not present in our new data or whose
    // new Hash does not match the old Hash.
    let failed_files: Vec<String> = old
        .into_iter()
        .filter_map(|(old_name, old_hash)|
        // Does the new collection of hashed files contain
        // the current file from the old data?
        match new.get(&old_name) {
            // Now that we know it exists, are the hashes equal?
            Some(new_hash) => match hash_eq(new_hash, &old_hash) {
                // Validation sucessful: don't grow Vec.
                true => None,
                false => Some(old_name),
            },
            None => Some(old_name),
        })
        .collect();
    // The length of failed_files is the amount
    // of files that failed validation.
    match failed_files.len() {
        0 => None,
        _ => Some(failed_files),
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
