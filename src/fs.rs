use crate::IOResult;
use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{Receiver, Sender};
use std::io;

const HIDDEN_ENTRY_PREFIX: char = '.';

/// Build a `Vec` containing the paths of all visible files within `dir_path`.
///
/// The ordering of these paths is non-deterministic.
#[inline(never)]
pub fn get_files(dir_path: &Utf8Path) -> IOResult<Vec<Utf8PathBuf>> {
    fn this_is_a_gyatt_function(
        dir_path: Utf8PathBuf,
        error_rx: Receiver<io::Error>,
        error_tx: Sender<io::Error>,
        path_tx: Sender<Utf8PathBuf>,
        scope: &rayon::Scope,
    ) {
        let entries = match dir_path.read_dir_utf8() {
            Ok(tmp) => tmp,
            Err(e) => {
                error_tx.send(e).unwrap();
                return;
            }
        };
        for entry in entries {
            // End early if some other thread has encountered an error.
            if !error_rx.is_empty() {
                return;
            }
            let entry = match entry {
                Ok(tmp) => tmp,
                Err(e) => {
                    error_tx.send(e).unwrap();
                    return;
                }
            };
            if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(tmp) => tmp,
                Err(e) => {
                    error_tx.send(e).unwrap();
                    return;
                }
            };
            // `Utf8PathBuf` is smaller than `Utf8DirEntry`.
            let path = entry.into_path();
            if metadata.is_file() {
                if metadata.len() > 0 {
                    path_tx.send(path).unwrap();
                }
            } else if metadata.is_dir() {
                let error_rx = error_rx.clone();
                let error_tx = error_tx.clone();
                let path_tx = path_tx.clone();
                scope.spawn(move |new_scope| {
                    this_is_a_gyatt_function(path, error_rx, error_tx, path_tx, new_scope)
                });
            }
        }
    }

    let (path_tx, path_rx) = crossbeam_channel::unbounded::<Utf8PathBuf>();
    let (error_tx, error_rx) = crossbeam_channel::unbounded::<io::Error>();

    // This **must** be cloned before being passed into the scope.
    // Otherwise `error_rx` will be moved into the scope and dropped when it
    // ends, causing our upcoming `recv` call to block indefinitely.
    let error_rx_clone = error_rx.clone();
    rayon::scope(move |scope| {
        this_is_a_gyatt_function(
            dir_path.to_path_buf(),
            error_rx_clone,
            error_tx,
            path_tx,
            scope,
        );
    });

    match error_rx.recv() {
        Err(_) => Ok(path_rx.into_iter().collect()),
        Ok(e) => Err(e),
    }
}

/*use crate::types::HashedFile;

/// Windows always has to be so funny and unique >:(
#[inline]
fn oi_vei(s: &str) -> String {
    const REPLACEMENT: char = '/';
    const WINDOWS_MOMENT: char = '\\';
    if cfg!(windows) {
        s.chars()
            .map(|c| match c == WINDOWS_MOMENT {
                false => c,
                true => REPLACEMENT,
            })
            .collect()
    } else {
        s.to_string()
    }
}

pub fn ayo_wtf(dir_path: &Utf8Path, prefix_len: usize) -> IOResult<Vec<HashedFile>> {
    fn hasher_function_with_nice_gyatt(
        file: Utf8PathBuf,
        prefix_len: usize,
        error_tx: Sender<io::Error>,
        hash_tx: Sender<HashedFile>,
    ) {
        let mut hasher = blake3::Hasher::new();
        let reader = match std::fs::File::open(file.as_std_path()) {
            Ok(tmp) => tmp,
            Err(e) => {
                error_tx.send(e).unwrap();
                return;
            }
        };
        // Calls to `Hasher::update_reader` internally buffer 64KiB of
        // data, so we don't need to worry about doing that manually.
        match hasher.update_reader(reader) {
            Ok(_) => {}
            Err(e) => {
                error_tx.send(e).unwrap();
                return;
            }
        };
        // SAFETY: Since all files are descendants of dir_path,
        // they all have dir_path as a prefix.
        let stripped_file_path = unsafe { file.as_str().get_unchecked(prefix_len..) };
        hash_tx
            .send(HashedFile {
                hash: hasher.finalize(),
                path: oi_vei(stripped_file_path),
                size: hasher.count(),
            })
            .unwrap();
    }

    fn this_is_a_gyatt_function(
        dir_path: Utf8PathBuf,
        prefix_len: usize,
        error_rx: Receiver<io::Error>,
        error_tx: Sender<io::Error>,
        hash_tx: Sender<HashedFile>,
        scope: &rayon::Scope,
    ) {
        let entries = match dir_path.read_dir_utf8() {
            Ok(tmp) => tmp,
            Err(e) => {
                error_tx.send(e).unwrap();
                return;
            }
        };
        for entry in entries {
            // End early if some other thread has encountered an error.
            if !error_rx.is_empty() {
                return;
            }
            let entry = match entry {
                Ok(tmp) => tmp,
                Err(e) => {
                    error_tx.send(e).unwrap();
                    return;
                }
            };
            if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(tmp) => tmp,
                Err(e) => {
                    error_tx.send(e).unwrap();
                    return;
                }
            };
            // `Utf8PathBuf` is smaller than `Utf8DirEntry`.
            let path = entry.into_path();
            if metadata.is_file() {
                if metadata.len() > 0 {
                    let error_tx = error_tx.clone();
                    let hash_tx = hash_tx.clone();
                    scope.spawn(move |new_scope| {
                        hasher_function_with_nice_gyatt(path, prefix_len, error_tx, hash_tx);
                    });
                }
            } else if metadata.is_dir() {
                let error_rx = error_rx.clone();
                let error_tx = error_tx.clone();
                let hash_tx = hash_tx.clone();
                scope.spawn(move |new_scope| {
                    this_is_a_gyatt_function(
                        path, prefix_len, error_rx, error_tx, hash_tx, new_scope,
                    )
                });
            }
        }
    }

    let (hash_tx, hash_rx) = crossbeam_channel::unbounded::<HashedFile>();
    let (error_tx, error_rx) = crossbeam_channel::unbounded::<io::Error>();

    // This **must** be cloned before being passed into the scope.
    // Otherwise `error_rx` will be moved into the scope and dropped when it
    // ends, causing our upcoming `recv` call to block indefinitely.
    let error_rx_clone = error_rx.clone();
    rayon::scope(move |scope| {
        this_is_a_gyatt_function(
            dir_path.to_path_buf(),
            prefix_len,
            error_rx_clone,
            error_tx,
            hash_tx,
            scope,
        );
    });

    match error_rx.recv() {
        Err(_) => Ok(hash_rx.into_iter().collect()),
        Ok(e) => Err(e),
    }
}*/
