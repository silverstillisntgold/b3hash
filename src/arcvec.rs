use std::sync::{Arc, Mutex};

/// Thin wrapper around a [`Vec`], forwarding just a small subset
/// of it's operations in a thread-safe way.
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
    /// Create a new `ArcVec` with a small default allocation.
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(4)
    }

    /// Call [`Vec::with_capacity`] with `capacity`.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let inner = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
        Self { inner }
    }

    /// Call [`Vec::is_empty`] after locking `self`.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    /// Call [`Vec::push`] with `value` after locking `self`.
    #[inline]
    pub fn push(&self, value: T) {
        self.inner.lock().unwrap().push(value);
    }

    /// Call [`Vec::extend`] with `iter` after locking `self`.
    #[inline]
    pub fn extend(&self, iter: impl IntoIterator<Item = T>) {
        self.inner.lock().unwrap().extend(iter);
    }

    /// Extract the internal `Vec` of this `ArcVec`.
    #[inline]
    pub fn into_inner(self) -> Vec<T> {
        Arc::into_inner(self.inner).unwrap().into_inner().unwrap()
    }
}
