use crate::hasher::DirectoryHasher;
use crate::manifest::{Entry, Manifest};
use crate::util::CancelHandle;
use bon::Builder;

#[derive(Builder)]
pub struct DirectoryVerifier<'a> {
    hasher: DirectoryHasher,
    manifest: &'a Manifest,
}

impl<'a> DirectoryVerifier<'a> {
    /// Attaches a [`CancelHandle`] to `self` for mid-process cancellation.
    /// Calling this multiple times will drop and override previous handles.
    pub fn cancel_handle(&mut self) -> CancelHandle {
        self.hasher.cancel_handle()
    }
}
