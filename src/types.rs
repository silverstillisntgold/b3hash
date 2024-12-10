use blake3::Hash;
use core::ops::Deref;

/// Result of hashing a directory.
///
/// Contains the name of the directory, the original `Vec` of
/// `HashedFile` instances, the cumulative hash representing all
/// visible data within the directory, and the cumulative size of
/// that data.
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

/// Result of hashing a file.
///
/// Contains the hash itself, the path from the root directory
/// to the hashed file, and the size of said file.
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
        self.path.as_str()
    }
}
