//! Utility `BTreeMap` that is upper-bounded in order to store the supervisor
//! workers (pool and checked in)
use std::{
    collections::{
        btree_map::Iter,
        BTreeMap,
    },
    num::NonZeroUsize,
};

use error::BoundedBTreeMapError;

/// Mostly transparent bounded map over a `BTreeMap` except over the `insert`
/// call which signals if we've reached the maps capacity limit.
#[derive(Debug)]
pub(crate) struct BoundedBTreeMap<K, V> {
    capacity: NonZeroUsize,
    inner: BTreeMap<K, V>,
}

impl<K, V> BoundedBTreeMap<K, V>
where
    K: Ord,
{
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            inner: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn iter(&self) -> Iter<K, V> {
        self.inner.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.capacity.into()
    }

    pub fn append(&mut self, map: &mut Self) {
        self.inner.append(&mut map.inner);
    }

    pub fn insert(&mut self, k: K, v: V) -> Result<(), BoundedBTreeMapError> {
        if self.inner.len() < self.capacity.into() {
            self.inner.insert(k, v);
            Ok(())
        } else {
            Err(BoundedBTreeMapError::CapacityLimit)
        }
    }

    pub fn pop_first(&mut self) -> Option<(K, V)> {
        self.inner.pop_first()
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        self.inner.remove(k)
    }

    pub fn remove_entry(&mut self, k: &K) -> Option<(K, V)> {
        self.inner.remove_entry(k)
    }
}

pub(crate) mod error {
    use std::{
        error::Error,
        fmt,
    };

    /// Error produced by the `BoundedBTreeMap`
    #[derive(PartialEq, Eq, Clone, Copy)]
    pub enum BoundedBTreeMapError {
        /// Bounded `BTreeMap` has reached it's capacity limit
        CapacityLimit,
    }

    impl fmt::Debug for BoundedBTreeMapError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("BoundedBTreeMapError")
                .finish_non_exhaustive()
        }
    }

    impl fmt::Display for BoundedBTreeMapError {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "BoundedBTreeMapError")
        }
    }

    impl Error for BoundedBTreeMapError {}
}
