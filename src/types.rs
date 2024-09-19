use blake3::Hash;
use core::ops::Deref;

/// TODO: docs
pub struct HashedDirectory {
    pub dir_name: String,
    pub files: Vec<HashedFile>,
    pub hash: Hash,
    /// Cumulative size of all hashed files, in bytes.
    pub size: u64,
}

impl Deref for HashedDirectory {
    type Target = [HashedFile];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.files
    }
}

/// TODO: docs
pub struct HashedFile {
    pub hash: Hash,
    pub path: String,
    /// Size of the hashed file, in bytes.
    pub size: u64,
}

impl Deref for HashedFile {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

// Used for building HashMap<String, Hash>
impl From<HashedFile> for (String, Hash) {
    #[inline]
    fn from(value: HashedFile) -> Self {
        (value.path, value.hash)
    }
}
