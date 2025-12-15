use crate::hasher::DirectoryHasher;
use blake3::Hash;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u64,
    pub directory_name: String,
    pub directory_hash: Hash,
    pub directory_size: u64,
    pub entries: Vec<Entry>,

    pub(crate) directory_hasher: DirectoryHasher,
}

#[derive(Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Entry {
    pub path: Utf8PathBuf,
    pub hash: Hash,
    pub size: u64,
}
