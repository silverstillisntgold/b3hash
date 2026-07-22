use crate::manifest::{Entry, Manifest};
use std::cmp::Ordering;

const CAPACITY: usize = 1 << 7;

/// Contains the differences between the two [`Manifest`]'s which were used to call [`verify`].
#[derive(Debug)]
pub struct DiffResult {
    old_only: Vec<Entry>,
    new_only: Vec<Entry>,
    changed: Vec<(Entry, Entry)>,
}

impl DiffResult {
    /// Returns entries which only existed in the old [`Manifest`].
    ///
    /// The entries in this slice are **missing** from the **new** `Manifest`.
    #[inline]
    pub fn old_only(&self) -> &[Entry] {
        &self.old_only
    }

    /// Returns entries which only existed in the new [`Manifest`].
    ///
    /// The entries in this slice are **missing** from the **old** `Manifest`.
    #[inline]
    pub fn new_only(&self) -> &[Entry] {
        &self.new_only
    }

    /// Returns entries which exist in both [`Manifest`]'s, but are different.
    ///
    /// The `path` field of each `Entry` pair in this slice will always be equal.
    #[inline]
    pub fn changed(&self) -> &[(Entry, Entry)] {
        &self.changed
    }
}

/// Compares the contents of two [`Manifest`]'s, returning `None` if they are the same.
///
/// If any difference is found, a [`DiffResult`] is returned which contains information about
/// how the two directories differ.
///
/// The only difference which is allowed is the name of the root directory, because the name of
/// a directory does not impact it's internal structure or the information it contains.
#[inline(never)]
pub fn verify(old_manifest: Manifest, new_manifest: Manifest) -> Option<DiffResult> {
    // Fast path: If both the total size and the hash of two directories are identical,
    // the probability of there being a mismatch of internal data between the two is so
    // astronomically tiny that so we assume they are the same.
    //
    // Size is compared first because it's the cheaper comparison, and is almost certainly going to
    // be different in cases where the hash is also different. We compare the bytes of the hash
    // instead of using the compare of the hash to take advantage of SIMD register compares.
    //
    // We've documented that it's fine for the name of the root directories to be different.
    if old_manifest.directory_size == new_manifest.directory_size
        && old_manifest.directory_hash.as_bytes() == new_manifest.directory_hash.as_bytes()
    {
        return None;
    }

    let mut result = DiffResult {
        old_only: Vec::with_capacity(CAPACITY),
        new_only: Vec::with_capacity(CAPACITY),
        changed: Vec::with_capacity(CAPACITY),
    };

    // These both need to be peekable because we need to be able to view the next entry
    // in the iterator, but we only want to consume it under specific conditions.
    let mut old_iter = old_manifest.entries.into_iter().peekable();
    let mut new_iter = new_manifest.entries.into_iter().peekable();

    while let (Some(old), Some(new)) = (old_iter.peek(), new_iter.peek()) {
        match old.path.cmp(&new.path) {
            Ordering::Equal => {
                // Even if they aren't different, both iterators must always be consumed here.
                let old = old_iter.next().unwrap();
                let new = new_iter.next().unwrap();
                // We're in this branch because we've already found the paths to be equal.
                if old.size != new.size || old.hash.as_bytes() != new.hash.as_bytes() {
                    result.changed.push((old, new));
                }
            }
            Ordering::Less => result.old_only.push(old_iter.next().unwrap()),
            Ordering::Greater => result.new_only.push(new_iter.next().unwrap()),
        }
    }

    // At most only one of these will actually append any data.
    result.old_only.extend(old_iter);
    result.new_only.extend(new_iter);

    Some(result)
}
