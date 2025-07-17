#![allow(unused)]

use crate::IOResult;
use blake3::{Hash, Hasher};
use camino::{Utf8Path, Utf8PathBuf};
use std::{fs::File, ops::Deref};

pub struct HashedFileV2 {
    pub hash: Hash,
    pub path: Utf8PathBuf,
    pub size: u64,
}

impl TryFrom<Utf8PathBuf> for HashedFileV2 {
    type Error = std::io::Error;

    fn try_from(value: Utf8PathBuf) -> Result<Self, Self::Error> {
        let path = value;
        let file_reader = File::open(path.as_std_path())?;
        let mut hasher = Hasher::new();
        hasher.update_reader(file_reader)?;
        let hash = hasher.finalize();
        let size = hasher.count();
        Ok(HashedFileV2 { hash, path, size })
    }
}

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
