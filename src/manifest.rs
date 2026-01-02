use blake3::Hash;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub directory_name: String,

    pub directory_hash: Hash,

    pub directory_size: u64,

    pub entries: Entries,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entries(Vec<Entry>);

impl Deref for Entries {
    type Target = [Entry];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl From<Vec<Entry>> for Entries {
    #[inline]
    fn from(value: Vec<Entry>) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub path: Utf8PathBuf,

    pub hash: Hash,

    pub size: u64,
}
