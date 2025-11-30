//! Test fixture for RustOwl performance benchmarks.
//!
//! Contains representative Rust patterns with ownership, borrowing, and
//! lifetime scenarios.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Container with ownership patterns for RustOwl to analyze.
#[derive(Debug, Clone)]
pub struct Container {
    pub id: String,
    pub data: Vec<u8>,
    pub metadata: HashMap<String, String>,
}

impl Container {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            data: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Returns a reference to internal data - ownership pattern.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable borrow pattern.
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// Potential panic with unwrap - intentional for testing.
    pub fn get_metadata_unchecked(&self, key: &str) -> &str {
        self.metadata.get(key).unwrap()
    }

    /// Safe alternative returning Option.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(String::as_str)
    }
}

/// Shared state with Arc<Mutex<T>> pattern.
#[derive(Clone)]
pub struct SharedState {
    inner: Arc<Mutex<Vec<Container>>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add(&self, container: Container) {
        self.inner.lock().unwrap().push(container);
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Process items with closure - lifetime pattern.
    pub fn process<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[Container]) -> R,
    {
        let guard = self.inner.lock().unwrap();
        f(&guard)
    }
}

/// Struct with lifetime parameter.
pub struct Borrowed<'a> {
    pub data: &'a [u8],
    pub name: &'a str,
}

impl<'a> Borrowed<'a> {
    pub fn new(data: &'a [u8], name: &'a str) -> Self {
        Self { data, name }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Iterator with lifetime - common pattern.
pub struct ContainerIter<'a> {
    containers: &'a [Container],
    index: usize,
}

impl<'a> ContainerIter<'a> {
    pub fn new(containers: &'a [Container]) -> Self {
        Self {
            containers,
            index: 0,
        }
    }
}

impl<'a> Iterator for ContainerIter<'a> {
    type Item = &'a Container;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.containers.len() {
            let item = &self.containers[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

/// Generate test data for benchmarking.
pub fn generate_dataset(count: usize) -> Vec<Container> {
    (0..count)
        .map(|i| {
            let mut c = Container::new(format!("item_{i}"));
            c.metadata.insert("index".into(), i.to_string());
            c.data = vec![i as u8; 64];
            c
        })
        .collect()
}

/// Recursive fibonacci - intentionally inefficient for CPU load.
pub fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container() {
        let c = Container::new("test").with_data(vec![1, 2, 3]);
        assert_eq!(c.data(), &[1, 2, 3]);
    }

    #[test]
    fn test_shared_state() {
        let state = SharedState::new();
        state.add(Container::new("a"));
        state.add(Container::new("b"));
        assert_eq!(state.count(), 2);
    }
    #[test]
    fn test_borrowed() {
        let data = [1, 2, 3];
        let b = Borrowed::new(&data, "test");
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn test_iterator() {
        let containers = generate_dataset(3);
        let iter = ContainerIter::new(&containers);
        assert_eq!(iter.count(), 3);
    }

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(10), 55);
    }
}
