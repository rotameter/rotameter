#![forbid(unsafe_code)]
//! `O(1)` arena-backed bounded LRU cache with zero `unsafe` code.

use std::collections::HashMap;
use std::hash::Hash;

struct Node<K, V> {
    key: K,
    val: V,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Fixed-capacity `O(1)` least-recently-used map backed by a `Vec` arena and `HashMap` index.
pub struct BoundedLru<K, V> {
    index: HashMap<K, usize>,
    nodes: Vec<Option<Node<K, V>>>,
    free: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    capacity: usize,
}

impl<K: Eq + Hash + Clone, V> BoundedLru<K, V> {
    /// Creates an empty LRU cache holding at most `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            index: HashMap::with_capacity(capacity.min(1_024)),
            nodes: Vec::with_capacity(capacity.min(1_024)),
            free: Vec::new(),
            head: None,
            tail: None,
            capacity,
        }
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Returns `true` when the cache holds zero entries.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Returns the maximum entry capacity configured for this cache.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Looks up `key`, promoting it to most-recently-used when found.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = *self.index.get(key)?;
        self.move_to_front(idx);
        self.nodes.get_mut(idx)?.as_mut().map(|node| &mut node.val)
    }

    /// Inserts `(key, val)`, updating in place on collision or evicting the LRU entry when full.
    pub fn insert(&mut self, key: K, val: V) -> Option<(K, V)> {
        if self.capacity == 0 {
            return Some((key, val));
        }
        if let Some(&idx) = self.index.get(&key) {
            if let Some(Some(node)) = self.nodes.get_mut(idx) {
                node.val = val;
            }
            self.move_to_front(idx);
            return None;
        }

        let evicted = (self.index.len() >= self.capacity)
            .then(|| self.pop_tail())
            .flatten();
        let idx = if let Some(reused) = self.free.pop() {
            self.nodes[reused] = Some(Node {
                key: key.clone(),
                val,
                prev: None,
                next: None,
            });
            reused
        } else {
            let next_idx = self.nodes.len();
            self.nodes.push(Some(Node {
                key: key.clone(),
                val,
                prev: None,
                next: None,
            }));
            next_idx
        };
        self.index.insert(key, idx);
        self.attach_front(idx);
        evicted
    }

    /// Removes `key` from the cache if present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let idx = self.index.remove(key)?;
        self.detach(idx);
        let node = self.nodes.get_mut(idx)?.take()?;
        self.free.push(idx);
        Some(node.val)
    }

    fn move_to_front(&mut self, idx: usize) {
        if self.head == Some(idx) {
            return;
        }
        self.detach(idx);
        self.attach_front(idx);
    }

    fn pop_tail(&mut self) -> Option<(K, V)> {
        let tail_idx = self.tail?;
        self.detach(tail_idx);
        let node = self.nodes.get_mut(tail_idx)?.take()?;
        self.index.remove(&node.key);
        self.free.push(tail_idx);
        Some((node.key, node.val))
    }

    fn attach_front(&mut self, idx: usize) {
        let old_head = self.head;
        if let Some(Some(node)) = self.nodes.get_mut(idx) {
            node.prev = None;
            node.next = old_head;
        }
        if let Some(h) = old_head
            && let Some(Some(head_node)) = self.nodes.get_mut(h)
        {
            head_node.prev = Some(idx);
        }
        self.head = Some(idx);
        if self.tail.is_none() {
            self.tail = Some(idx);
        }
    }

    fn detach(&mut self, idx: usize) {
        let (prev, next) = match self.nodes.get(idx).and_then(Option::as_ref) {
            Some(node) => (node.prev, node.next),
            None => return,
        };
        if let Some(p) = prev {
            if let Some(Some(prev_node)) = self.nodes.get_mut(p) {
                prev_node.next = next;
            }
        } else {
            self.head = next;
        }
        if let Some(n) = next {
            if let Some(Some(next_node)) = self.nodes.get_mut(n) {
                next_node.prev = prev;
            }
        } else {
            self.tail = prev;
        }
        if let Some(Some(node)) = self.nodes.get_mut(idx) {
            node.prev = None;
            node.next = None;
        }
    }
}
