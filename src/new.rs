use std::num::NonZeroUsize;

pub struct Entry {
    path: camino::Utf8PathBuf,
    size: u64,
    hash: blake3::Hash,
}

pub struct Manifest {
    version: u64,
    entries: Vec<Entry>,
}

pub struct HashedDirectory;

#[derive(bon::Builder)]
pub struct DirectoryHasher {
    custom_ignore_source: Option<String>,

    num_threads: Option<NonZeroUsize>,

    #[builder(default = false)]
    follow_symlinks: bool,

    #[builder(default = true)]
    respect_hidden: bool,

    #[builder(default = true)]
    respect_ignore: bool,
}

impl DirectoryHasher {
    pub fn hash(self) -> HashedDirectory {
        todo!()
    }
}
