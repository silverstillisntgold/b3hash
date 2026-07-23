use crate::manifest::{Entry, Manifest};
use std::cmp::Ordering;

const CAPACITY: usize = 1 << 7;

/// Contains the differences between the two [`Manifest`]'s which were used to call [`verify`].
#[derive(Debug)]
pub struct DirectoryDiff {
    old_only: Vec<Entry>,
    new_only: Vec<Entry>,
    changed: Vec<(Entry, Entry)>,
}

impl DirectoryDiff {
    /// Returns the entries which only existed in the old [`Manifest`].
    #[inline]
    pub fn old_only(&self) -> &[Entry] {
        &self.old_only
    }

    /// Returns the entries which only existed in the new [`Manifest`].
    #[inline]
    pub fn new_only(&self) -> &[Entry] {
        &self.new_only
    }

    /// Returns the entries which existed in both [`Manifest`]'s, but were different.
    ///
    /// The `path` field of each [`Entry`] pair in this slice will always be equal.
    #[inline]
    pub fn changed(&self) -> &[(Entry, Entry)] {
        &self.changed
    }
}

/// Compares the contents of two [`Manifest`]'s, returning [`None`] if they are the same.
///
/// If any difference is found, a [`DirectoryDiff`] is returned which contains information about
/// how the two directories differ.
///
/// Different root directory names/paths are allowed, because the name/path of a directory
/// does not impact it's internal structure or the information it contains.
#[inline(never)]
pub fn verify(old_manifest: Manifest, new_manifest: Manifest) -> Option<DirectoryDiff> {
    // Fast path: If both the total size and the hash of two directories are identical,
    // the probability of there being a mismatch of internal data between the two is so
    // astronomically tiny that we can safely assume they are the same.
    //
    // Size is compared first because it's the cheaper comparison, and is almost certainly going to
    // be different in cases where the hash is also different. We compare the bytes of the hash
    // instead of using the compare of the hash to take advantage of SIMD register compares.
    // The specialized hash compare is optimized for cryptographic security we don't have need for,
    // so it may not generate the fastest assembly on all platforms.
    //
    // We've documented that it's fine for the name/path of the root directories to be different.
    if old_manifest.directory_size == new_manifest.directory_size
        && old_manifest.directory_hash.as_bytes() == new_manifest.directory_hash.as_bytes()
    {
        return None;
    }

    // If the `directory_size` and `directory_hash` values are not the same, then we know for
    // certain that there **must** be some kind of difference between the two manifests.

    let mut result = DirectoryDiff {
        old_only: Vec::with_capacity(CAPACITY),
        new_only: Vec::with_capacity(CAPACITY),
        changed: Vec::with_capacity(CAPACITY),
    };

    // These both need to be peekable because we need to be able to view the next entry
    // in the iterators, but we only want to consume them under specific conditions.
    let mut old_iter = old_manifest.entries.into_iter().peekable();
    let mut new_iter = new_manifest.entries.into_iter().peekable();

    while let (Some(old), Some(new)) = (old_iter.peek(), new_iter.peek()) {
        match old.path().cmp(new.path()) {
            Ordering::Equal => {
                // Because the paths are equal both iterators are currently at the same
                // position, and they must both be consumed to keep them in lockstep.
                let old = old_iter.next().unwrap();
                let new = new_iter.next().unwrap();
                // We're just checking for size or hash differences.
                if old.size != new.size || old.hash.as_bytes() != new.hash.as_bytes() {
                    result.changed.push((old, new));
                }
            }
            Ordering::Less => result.old_only.push(old_iter.next().unwrap()),
            Ordering::Greater => result.new_only.push(new_iter.next().unwrap()),
        }
    }

    // At most only one of these will append any data.
    result.old_only.extend(old_iter);
    result.new_only.extend(new_iter);

    Some(result)
}
