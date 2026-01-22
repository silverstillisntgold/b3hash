use crate::{HASHFILE, util::SerdeError};
use blake3::Hash;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    /// Contains the original path of the directory being hashed.
    /// Only contains `Some` when created from a directory hasher.
    #[serde(skip)]
    pub(crate) directory_path: Option<Utf8PathBuf>,
    pub(crate) directory_name: String,
    pub(crate) directory_hash: Hash,
    pub(crate) directory_size: u64,
    pub(crate) entries: Vec<Entry>,
}

impl Manifest {
    #[inline(never)]
    pub fn serialize(self) -> Result<bool, SerdeError> {
        /// Use zstd's default compression level.
        const COMPRESSION_LEVEL: i32 = 0;
        match &self.directory_path {
            Some(path) => {
                let path = path.join(HASHFILE);
                let source = serde_json::to_vec(&self)?;
                let contents = zstd::encode_all(source.as_slice(), COMPRESSION_LEVEL)?;
                fs::write(path, contents)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    #[inline]
    pub fn deserialize<T>(path: T) -> Result<Manifest, SerdeError>
    where
        T: AsRef<Utf8Path>,
    {
        Self::deserialize_internal(path.as_ref())
    }

    #[inline(never)]
    fn deserialize_internal(path: &Utf8Path) -> Result<Manifest, SerdeError> {
        let path = path.join(HASHFILE);
        let contents = fs::read(path)?;
        let contents = zstd::decode_all(contents.as_slice())?;
        serde_json::from_slice(&contents).map_err(Into::into)
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

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
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
