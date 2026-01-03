use crate::HASHFILE;
use crate::util::Error;
use blake3::Hash;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::fs;
use std::ops::Deref;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    /// Contains the original path relative to program execution where
    /// the directory being hashed is.
    #[serde(skip)]
    pub(crate) directory_path: Option<Utf8PathBuf>,

    pub directory_name: String,

    pub directory_hash: Hash,

    pub directory_size: u64,

    pub entries: Entries,
}

impl Manifest {
    pub fn serialize(self) -> Result<bool, Error> {
        if let Some(path) = &self.directory_path {
            let path = path.join(HASHFILE);
            let s = serde_json::to_vec(&self)?;
            fs::write(path, s)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn deserialize(path: Utf8PathBuf) -> Result<Manifest, Error> {
        let path = path.join(HASHFILE);
        let s = fs::read(path)?;
        let m = serde_json::from_slice(&s)?;
        Ok(m)
    }
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
