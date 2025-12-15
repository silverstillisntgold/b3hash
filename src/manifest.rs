use crate::hasher::DirectoryHasher;
use crate::util::Error;
use blake3::Hash;
use camino::Utf8PathBuf;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u64,
    pub directory_name: String,
    pub directory_hash: Hash,
    pub directory_size: u64,
    pub entries: Vec<Entry>,

    pub(crate) directory_hasher: DirectoryHasher,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Entry {
    pub path: Utf8PathBuf,
    pub hash: Hash,
    pub size: u64,
}

impl Manifest {
    #[inline(never)]
    pub fn verify(&self) -> Result<Option<Vec<Entry>>, Error> {
        let old_entries = &self.entries;
        let new_entries = self.directory_hasher.hash_internal::<HashSet<Entry>>()?;
        let missing_entries = old_entries
            .par_iter()
            .filter(|entry| !new_entries.contains(entry))
            .cloned()
            .collect::<Vec<Entry>>();
        Ok(match missing_entries.len() {
            0 => None,
            _ => Some(missing_entries),
        })
    }
}
