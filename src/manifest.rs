use crate::hasher::DirectoryHasher;
use crate::util::{CancelHandle, Error, Event};
use blake3::Hash;
use camino::Utf8PathBuf;
use crossbeam_channel::Sender;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub path: Utf8PathBuf,
    pub hash: Hash,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u64,
    pub directory_name: String,
    pub directory_hash: Hash,
    pub directory_size: u64,
    pub entries: Vec<Entry>,

    pub(crate) directory_hasher: DirectoryHasher,
}

impl Manifest {
    pub fn cancel_handle(&mut self) -> CancelHandle {
        self.directory_hasher.cancel_handle()
    }

    pub fn progress_channel(&mut self, sender: Sender<Event>) {
        self.directory_hasher.progress_channel = Some(sender);
    }

    #[inline(never)]
    pub fn verify(&self) -> Result<Option<Vec<Entry>>, Error> {
        let old_entries = &self.entries;
        let new_entries = self.directory_hasher.hash_entries()?;
        let missing_entries = old_entries
            .par_iter()
            .filter(|entry| {
                new_entries
                    .binary_search_by(|e| e.path.cmp(&entry.path))
                    .is_err()
            })
            .cloned()
            .collect::<Vec<Entry>>();
        Ok(match missing_entries.len() {
            0 => None,
            _ => Some(missing_entries),
        })
    }

    #[inline(never)]
    pub fn verify_entries(&self, entries: &[Entry]) -> Result<Option<Vec<Entry>>, Error> {
        _ = entries;
        todo!()
    }
}
