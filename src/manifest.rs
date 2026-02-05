use crate::{HASHFILE, SerdeError};
use blake3::Hash;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;

const COMPRESSION_LEVEL: i32 = zstd::DEFAULT_COMPRESSION_LEVEL;

/// The result of hashing a [`DirectoryHasher`](crate::hasher::DirectoryHasher)
/// or consuming the entirety of a [`DirectoryHasherIter`](crate::hasher::DirectoryHasherIter).
///
/// Can be serialized into a b3hash file with [`Self::serialize`], or used to verify against
/// another [`Manifest`] using [`verify`](crate::verifier::verify).
///
/// If a `Manifest` instance is the result of calling [`Self::deserialize`], then it is
/// not possible to reserialize it. That is to say that [`Manifest`]'s can only be serialized
/// when they come directly from `DirectoryHasher` or it's iterator.
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
    /// Attempts to serialize `self` into a new b3hash file, returning `Ok(true)` on success.
    ///
    /// If `self` was derived from any source other than a [`DirectoryHasher`](crate::hasher::DirectoryHasher),
    /// this will always return `Ok(false)`.
    #[inline(never)]
    pub fn serialize(self) -> Result<bool, SerdeError> {
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

    /// Atttempts to deserialize the contents of the b3hash file within the `path`
    /// directory and returns the resulting [`Manifest`].
    ///
    /// # Warning
    ///
    /// The [`Manifest`] which is returned from this method **cannot** be
    /// used to serialize a new b3hash file.
    #[inline]
    pub fn deserialize<T>(path: T) -> Result<Manifest, SerdeError>
    where
        T: AsRef<Utf8Path>,
    {
        Self::deserialize_internal(path.as_ref())
    }

    /// Internal use, see [`Self::deserialize`] for documentation.
    #[inline(never)]
    fn deserialize_internal(path: &Utf8Path) -> Result<Manifest, SerdeError> {
        let path = path.join(HASHFILE);
        let contents = fs::read(path)?;
        let contents = zstd::decode_all(contents.as_slice())?;
        serde_json::from_slice(&contents).map_err(Into::into)
    }

    /// Returns the name of the directory.
    ///
    /// This is the name of the root directory which was hashed to generate this [`Manifest`].
    #[inline]
    pub fn name(&self) -> &str {
        &self.directory_name
    }

    /// Returns the hash of the directory.
    ///
    /// This is the cumulative hash of all [`Entry`]'s within the original root directory.
    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.directory_hash
    }

    /// Returns the size of the directory, in bytes.
    ///
    /// This is the cumulative size of all files within the original directory.
    #[inline]
    pub fn size(&self) -> u64 {
        self.directory_size
    }

    /// Returns a slice containing all [`Entry`]'s which make up this [`Manifest`].
    ///
    /// This slice is always sorted by the `path` field of each entry.
    #[inline]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

/// Struct containing the data of a hashed file.
///
/// Only exists within the context of a parent [`Manifest`].
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub(crate) path: Utf8PathBuf,
    pub(crate) hash: Hash,
    pub(crate) size: u64,
}

impl Entry {
    /// Returns the path of this [`Entry`], relative to the root directory,
    /// but stripped of that directories name.
    #[inline]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Returns the hash of this [`Entry`].
    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Returns the size of this [`Entry`], in bytes.
    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }
}
