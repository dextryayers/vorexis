use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Simple thread-safe TTL cache used to deduplicate work across modules
/// (HTTP responses, DNS lookups, visited URLs, ...).
pub struct Cache {
    inner: Mutex<HashMap<String, (Instant, String)>>,
    ttl: Duration,
    hits: Mutex<u64>,
}

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        Cache {
            inner: Mutex::new(HashMap::new()),
            ttl,
            hits: Mutex::new(0),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some((at, val)) = inner.get(key) {
            if at.elapsed() < self.ttl {
                *self.hits.lock().unwrap() += 1;
                return Some(val.clone());
            }
            inner.remove(key);
        }
        None
    }

    pub fn set(&self, key: &str, value: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.insert(key.to_string(), (Instant::now(), value));
        if inner.len() > 10_000 {
            let ttl = self.ttl;
            inner.retain(|_, (at, _)| at.elapsed() < ttl);
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Deduplication helper: returns true if the key is new, false if seen.
    pub fn dedup(&self, key: &str) -> bool {
        if self.contains(key) {
            false
        } else {
            self.set(key, String::new());
            true
        }
    }

    pub fn stats(&self) -> (usize, u64) {
        (self.inner.lock().unwrap().len(), *self.hits.lock().unwrap())
    }
}
