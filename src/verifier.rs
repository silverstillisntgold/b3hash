use crate::hasher::DirectoryHasher;
use crate::manifest::{Entries, Entry};
use crate::send_if_channel;
use crate::util::{CancelHandle, Error, Event};
use std::cmp::Ordering;

const DEFAULT_CAP: usize = 1 << 7;

#[derive(Debug)]
pub struct DiffResult {
    pub old_only: Vec<Entry>,

    pub new_only: Vec<Entry>,

    pub changed: Vec<(Entry, Entry)>,
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
    pub fn is_identical(&self) -> bool {
        self.old_only.is_empty() && self.new_only.is_empty() && self.changed.is_empty()
    }
}

#[derive(bon::Builder)]
pub struct DirectoryVerifier<'a> {
    hasher: DirectoryHasher,
    old_entries: &'a Entries,
}

impl<'a> DirectoryVerifier<'a> {
    /// Calls [`DirectoryHasher::cancel_handle`] on the internal [`DirectoryHasher`].
    pub fn cancel_handle(&mut self) -> CancelHandle {
        self.hasher.cancel_handle()
    }

    #[inline(never)]
    pub fn verify(self) -> Result<DiffResult, Error> {
        let mut result = DiffResult::new();

        let mut old_iter = self.old_entries.iter().peekable();
        let mut new_iter = self.hasher.hash_entries()?.into_iter().peekable();

        send_if_channel!(
            self.hasher.progress_channel,
            Event::DirectoryVerificationStarted
        );
        loop {
            let case = match (old_iter.peek(), new_iter.peek()) {
                (Some(old), Some(new)) => Some(old.path.cmp(&new.path)),
                (Some(_), None) => Some(Ordering::Less),
                (None, Some(_)) => Some(Ordering::Greater),
                (None, None) => None,
            };
            match case {
                Some(Ordering::Equal) => {
                    let old = unsafe { old_iter.next().unwrap_unchecked() };
                    let new = unsafe { new_iter.next().unwrap_unchecked() };
                    if old != &new {
                        result.changed.push((old.clone(), new));
                    }
                }
                Some(Ordering::Less) => {
                    let entry = unsafe { old_iter.next().unwrap_unchecked() };
                    result.old_only.push(entry.clone());
                }
                Some(Ordering::Greater) => {
                    let entry = unsafe { new_iter.next().unwrap_unchecked() };
                    result.new_only.push(entry);
                }
                None => break,
            }
        }
        send_if_channel!(
            self.hasher.progress_channel,
            Event::DirectoryVerificationCompleted(result.is_identical())
        );

        Ok(result)
    }
}
