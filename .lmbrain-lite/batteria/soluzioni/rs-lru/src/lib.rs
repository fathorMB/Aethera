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

    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            if let Some(k) = self.order.remove(pos) {
                self.order.push_back(k);
            }
        }
    }

    /// Returns the value and marks the key as the most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if !self.map.contains_key(key) {
            return None;
        }
        self.touch(key);
        self.map.get(key)
    }

    /// Inserts or updates a value; the key becomes the most recently used.
    ///
    /// When a new key does not fit, the least recently used entry is evicted and returned.
    /// Updating an existing key never evicts anything and returns `None`.
    /// With capacity zero nothing is stored and `None` is returned.
    pub fn put(&mut self, key: K, value: V) -> Option<(K, V)> {
        if self.capacity == 0 {
            return None;
        }
        if self.map.contains_key(&key) {
            self.touch(&key);
            self.map.insert(key, value);
            return None;
        }
        let mut evicted = None;
        if self.map.len() >= self.capacity {
            if let Some(old) = self.order.pop_front() {
                let v = self.map.remove(&old).expect("order and map agree");
                evicted = Some((old, v));
            }
        }
        self.order.push_back(key.clone());
        self.map.insert(key, value);
        evicted
    }

    /// Removes a key and returns its value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let value = self.map.remove(key)?;
        self.order.retain(|k| k != key);
        Some(value)
    }
}
