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
    let mut result = DiffResult::new();

    if old_manifest.directory_size == new_manifest.directory_size
        && old_manifest.directory_hash == new_manifest.directory_hash
    {
        return None;
    }

    let mut old_iter = old_manifest.entries.into_iter().peekable();
    let mut new_iter = new_manifest.entries.into_iter().peekable();

    loop {
        let ord = match (old_iter.peek(), new_iter.peek()) {
            (Some(old), Some(new)) => old.path.cmp(&new.path),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        match ord {
            Ordering::Equal => {
                let old = old_iter.next().unwrap();
                let new = new_iter.next().unwrap();
                if old != new {
                    result.changed.push((old, new));
                }
            }
            Ordering::Less => {
                let entry = old_iter.next().unwrap();
                result.old_only.push(entry);
            }
            Ordering::Greater => {
                let entry = new_iter.next().unwrap();
                result.new_only.push(entry);
            }
        }
    }

    match result.is_identical() {
        true => None,
        false => Some(result),
    }
}
