use blake3::Hash;
use std::ops::Deref;

pub struct HashedDirectory {
    pub name: String,
    pub files: Vec<HashedFile>,
    pub hash: Hash,
    /// Cumulative size of all hashed files, in bytes.
    pub size: u64,
}

impl Deref for HashedDirectory {
    type Target = [HashedFile];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.files.as_slice()
    }
}

pub struct HashedFile {
    pub hash: Hash,
    pub path: String,
    pub size: u64,
}

impl Deref for HashedFile {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.path.as_str()
    }
}
