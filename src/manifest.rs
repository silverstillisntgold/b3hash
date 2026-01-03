use crate::HASHFILE;
use crate::util::Error;
use blake3::Hash;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    /// Contains the original path of the directory being hashed.
    #[serde(skip)]
    pub(crate) directory_path: Option<Utf8PathBuf>,

    pub(crate) directory_name: String,

    pub(crate) directory_hash: Hash,

    pub(crate) directory_size: u64,

    pub(crate) entries: Vec<Entry>,
}

impl Manifest {
    #[inline(never)]
    pub fn serialize_pretty(self) -> Result<bool, Error> {
        self.serialize_internal::<true>()
    }

    #[inline(never)]
    pub fn serialize(self) -> Result<bool, Error> {
        self.serialize_internal::<false>()
    }

    #[inline]
    fn serialize_internal<const PRETTY: bool>(self) -> Result<bool, Error> {
        if let Some(path) = &self.directory_path {
            let path = path.join(HASHFILE);
            let data = if PRETTY {
                serde_json::to_vec_pretty(&self)
            } else {
                serde_json::to_vec(&self)
            }?;
            fs::write(path, data)?;
            return Ok(true);
        }
        Ok(false)
    }

    #[inline(never)]
    pub fn deserialize(path: Utf8PathBuf) -> Result<Manifest, Error> {
        let path = path.join(HASHFILE);
        let data = fs::read(path)?;
        serde_json::from_slice(&data).map_err(|e| e.into())
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.directory_name
    }

    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.directory_hash
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.directory_size
    }

    #[inline]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub(crate) path: Utf8PathBuf,

    pub(crate) hash: Hash,

    pub(crate) size: u64,
}

impl Entry {
    #[inline]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }
}
