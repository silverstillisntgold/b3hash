use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

const DEFAULT_CAPACITY: usize = 4;

/// Thin, thread-safe wrapper around a [`Vec`].
pub struct ArcVec<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> Clone for ArcVec<T> {
    /// Makes a clone of the internal `Arc` pointer, increasing the strong reference count.
    #[inline]
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<T> ArcVec<T> {
    /// Creates a new `ArcVec` with a small default allocation.
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Calls [`Vec::with_capacity`] with `capacity`.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let inner = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
        Self { inner }
    }

    /// Provides access to the inner `Vec` via a [`MutexGuard`].
    #[inline]
    pub fn inner<'a>(&'a self) -> MutexGuard<'a, Vec<T>> {
        self.inner.lock()
    }

    /// Extracts the internal `Vec` of this `ArcVec`.
    ///
    /// SAFETY: The strong reference count of `self` must be 1.
    #[inline]
    pub unsafe fn into_inner(self) -> Vec<T> {
        unsafe { Arc::into_inner(self.inner).unwrap_unchecked().into_inner() }
    }
}
