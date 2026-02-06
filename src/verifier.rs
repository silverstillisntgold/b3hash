use crate::manifest::{Entry, Manifest};
use std::cmp::Ordering;

const DEFAULT_CAP: usize = 1 << 7;

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
    /// The entries in this slice are **missing** from the new `Manifest`.
    #[inline]
    pub fn old_only(&self) -> &[Entry] {
        &self.old_only
    }

    /// Returns entries which only existed in the new [`Manifest`].
    ///
    /// The entries in this slice are **missing** from the old `Manifest`.
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
    // It's fine if the name of the root directories are different.
    if old_manifest.directory_size == new_manifest.directory_size
        && old_manifest.directory_hash.as_bytes() == new_manifest.directory_hash.as_bytes()
    {
        return None;
    }

    let mut result = DiffResult {
        old_only: Vec::with_capacity(DEFAULT_CAP),
        new_only: Vec::with_capacity(DEFAULT_CAP),
        changed: Vec::with_capacity(DEFAULT_CAP),
    };
    let mut old_iter = old_manifest.entries.into_iter().peekable();
    let mut new_iter = new_manifest.entries.into_iter().peekable();

    while let (Some(old), Some(new)) = (old_iter.peek(), new_iter.peek()) {
        match old.path.cmp(&new.path) {
            Ordering::Equal => {
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
