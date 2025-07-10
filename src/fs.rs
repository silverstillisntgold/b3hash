use crate::IOResult;
use camino::Utf8PathBuf;
use crossbeam_channel::{Receiver, Sender};
use std::io;

const HIDDEN_ENTRY_PREFIX: char = '.';

/// Build a `Vec` containing the paths of all visible files within `dir_path`.
///
/// This is done in such a way that each directory spawns a distinct rayon task,
/// containing it's own sequential iterator over it's contents. Valid file paths are sent
/// through a channel to later be collected into the returned `Vec`. If an error is
/// encountered, each rayon task will terminate as soon as possible and the error
/// will be propagated to the caller.
///
/// The ordering of paths in the returned `Vec` is non-deterministic.
#[inline(never)]
pub fn get_file_paths(dir_path: Utf8PathBuf) -> IOResult<Vec<Utf8PathBuf>> {
    let (path_tx, path_rx) = crossbeam_channel::unbounded::<Utf8PathBuf>();
    let (error_tx, error_rx) = crossbeam_channel::unbounded::<io::Error>();

    // Need to pass a cloned instance into the scope, or the original
    // `error_rx` will be dropped before we need to use it.
    // This doesn't need to be done for the senders because we want them
    // to be dropped/closed when all work is completed.
    let error_rx_clone = error_rx.clone();
    rayon::in_place_scope(move |scope| {
        this_is_a_gyatt_function(dir_path, error_rx_clone, error_tx, path_tx, scope);
    });

    // No need to use blocking operations because both senders
    // will have been dropped/closed by this point.
    match error_rx.try_recv() {
        // An `Err` here means that our error channel
        // is empty, which is what we want.
        Err(_) => Ok(path_rx.into_iter().collect()),
        Ok(e) => Err(e),
    }
}

macro_rules! unwrap_or_send_error {
    ($expr: expr, $err_chan: ident) => {
        match $expr {
            Ok(value) => value,
            Err(e) => {
                $err_chan.send(e).unwrap();
                return;
            }
        }
    };
}

fn this_is_a_gyatt_function(
    dir_path: Utf8PathBuf,
    error_rx: Receiver<io::Error>,
    error_tx: Sender<io::Error>,
    path_tx: Sender<Utf8PathBuf>,
    scope: &rayon::Scope,
) {
    let entries = unwrap_or_send_error!(dir_path.read_dir_utf8(), error_tx);
    for entry in entries {
        // Terminate early if some other worker has
        // already sent out an error.
        if !error_rx.is_empty() {
            return;
        }
        let entry = unwrap_or_send_error!(entry, error_tx);
        if entry.file_name().starts_with(HIDDEN_ENTRY_PREFIX) {
            continue;
        }
        let metadata = unwrap_or_send_error!(entry.metadata(), error_tx);
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
