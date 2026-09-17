//! A small least-recently-used cache, std only.
//!
//! Recency is kept in `order`: the front is the least recently used key, the back the most
//! recently used. `map` holds the values. The two must always contain the same keys.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

pub struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// A cache holding at most `capacity` entries. A capacity of zero stores nothing.
    pub fn new(capacity: usize) -> Self {
        Self { capacity, map: HashMap::new(), order: VecDeque::new() }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Looks at a value without changing its recency.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Keys from least to most recently used.
    pub fn keys_by_recency(&self) -> Vec<K> {
        self.order.iter().cloned().collect()
    }

    /// Returns the value and marks the key as the most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let _ = key;
        todo!("get")
    }

    /// Inserts or updates a value; the key becomes the most recently used.
    ///
    /// When a new key does not fit, the least recently used entry is evicted and returned.
    /// Updating an existing key never evicts anything and returns `None`.
    /// With capacity zero nothing is stored and `None` is returned.
    pub fn put(&mut self, key: K, value: V) -> Option<(K, V)> {
        let _ = (key, value);
        todo!("put")
    }

    /// Removes a key and returns its value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let value = self.map.remove(key)?;
        self.order.retain(|k| k != key);
        Some(value)
    }
}
