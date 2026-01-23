use crate::manifest::{Entry, Manifest};
use std::cmp::Ordering;

const DEFAULT_CAP: usize = 1 << 7;

#[derive(Debug)]
pub struct DiffResult {
    old_only: Vec<Entry>,

    new_only: Vec<Entry>,

    changed: Vec<(Entry, Entry)>,
}

impl DiffResult {
    pub(crate) fn new() -> Self {
        Self {
            old_only: Vec::with_capacity(DEFAULT_CAP),
            new_only: Vec::with_capacity(DEFAULT_CAP),
            changed: Vec::with_capacity(DEFAULT_CAP),
        }
    }

    /// Returns true if both vectors are identical.
    pub(crate) fn is_identical(&self) -> bool {
        self.old_only.is_empty() && self.new_only.is_empty() && self.changed.is_empty()
    }
}

#[inline(never)]
pub fn verify(old_manifest: Manifest, new_manifest: Manifest) -> Option<DiffResult> {
    if old_manifest.directory_size == new_manifest.directory_size
        && old_manifest.directory_hash == new_manifest.directory_hash
    {
        return None;
    }

    let mut result = DiffResult::new();
    let mut old_iter = old_manifest.entries.into_iter().peekable();
    let mut new_iter = new_manifest.entries.into_iter().peekable();

    while let (Some(old), Some(new)) = (old_iter.peek(), new_iter.peek()) {
        match old.path().cmp(new.path()) {
            Ordering::Equal => {
                let old = old_iter.next().unwrap();
                let new = new_iter.next().unwrap();
                if old != new {
                    result.changed.push((old, new));
                }
            }
            Ordering::Less => result.old_only.push(old_iter.next().unwrap()),
            Ordering::Greater => result.new_only.push(new_iter.next().unwrap()),
        }
    }
    result.old_only.extend(old_iter);
    result.new_only.extend(new_iter);

    match result.is_identical() {
        true => None,
        false => Some(result),
    }
}
