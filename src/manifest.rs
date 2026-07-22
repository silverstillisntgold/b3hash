use crate::{HASHFILE, SerdeError};
use blake3::Hash;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;

// For doc links.
#[allow(unused)]
use crate::hasher::{DirectoryHasher, DirectoryHasherIter};

const COMPRESSION_LEVEL: i32 = zstd::DEFAULT_COMPRESSION_LEVEL;

/// The result of calling [`DirectoryHasher::hash`], or consuming the entirety of a [`DirectoryHasherIter`].
///
/// Can be serialized into a b3hash file with [`Self::serialize`], or used to verify against
/// another [`Manifest`] using [`verify`](crate::verifier::verify).
///
/// If a `Manifest` instance is the result of calling [`Self::deserialize`], then it is
/// not possible to reserialize it. That is to say that [`Manifest`]'s can only be serialized
/// when they come directly from a [`DirectoryHasher`] or it's iterator.
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
    /// If `self` was derived from any source other than an original [`DirectoryHasher`]
    /// or it's iterator, this will always return `Ok(false)`.
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
        let source = zstd::decode_all(contents.as_slice())?;
        serde_json::from_slice(&source).map_err(Into::into)
    }

    /// Returns the path which was used to create `self`.
    ///
    /// This will only be `Some` if `self` comes from a [`DirectoryHasher`] or
    /// [`DirectoryHasherIter`]. It will always be fully canonicalized.
    #[inline]
    pub fn path(&self) -> Option<&Utf8Path> {
        self.directory_path.as_deref()
    }

    /// Returns the name of `self`.
    ///
    /// This is the name of the root directory which was hashed to generate `self`.
    #[inline]
    pub fn name(&self) -> &str {
        &self.directory_name
    }

    /// Returns the hash of `self`.
    ///
    /// This is the cumulative hash of all [`Entry`]'s within the root directory.
    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.directory_hash
    }

    /// Returns the size of `self`, in bytes.
    ///
    /// This is the cumulative size of all [`Entry`]'s within the root directory.
    #[inline]
    pub fn size(&self) -> u64 {
        self.directory_size
    }

    /// Returns a slice containing all [`Entry`]'s which make up `self`.
    ///
    /// The returned slice is sorted by the `path` field of each [`Entry`].
    #[inline]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

/// Struct containing the data of a hashed file.
///
/// Only exists within a parent [`Manifest`].
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub(crate) path: Utf8PathBuf,
    pub(crate) hash: Hash,
    pub(crate) size: u64,
}

impl Entry {
    /// Returns the path of `self`, relative to the root directory,
    /// but stripped of that root directories name.
    #[inline]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Returns the hash of `self`.
    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Returns the size of `self`, in bytes.
    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }
}
