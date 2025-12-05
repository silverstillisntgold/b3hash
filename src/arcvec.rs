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
        let inner = Arc::clone(&self.inner);
        Self { inner }
    }
}

impl<T> ArcVec<T> {
    /// Constructs a new, empty `ArcVec` with a small default allocation.
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Constructs a new, empty `ArcVec` with at least the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let inner = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
        Self { inner }
    }

    /// Acquires a mutex on the the inner `Vec`, blocking the current
    /// thread until it is able to do so.
    #[inline]
    pub fn lock<'a>(&'a self) -> MutexGuard<'a, Vec<T>> {
        self.inner.lock()
    }

    /// Returns the inner `Vec` of this `ArcVec`.
    ///
    /// SAFETY: The strong reference count of `self` must be 1.
    #[inline]
    pub unsafe fn into_inner(self) -> Vec<T> {
        debug_assert!(!self.inner.is_locked() && Arc::strong_count(&self.inner) == 1);
        unsafe { Arc::into_inner(self.inner).unwrap_unchecked().into_inner() }
    }
}
