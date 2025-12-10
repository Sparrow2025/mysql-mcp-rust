use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// A simple time-based cache with TTL (Time To Live)
/// Uses LRU-like eviction when capacity is reached
#[derive(Debug)]
pub struct TtlCache<K, V> {
    entries: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    ttl: Duration,
    max_capacity: usize,
}

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
}

impl<V> CacheEntry<V> {
    fn new(value: V) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Create a new TTL cache with specified TTL and maximum capacity
    pub fn new(ttl: Duration, max_capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_capacity,
        }
    }

    /// Get a value from the cache if it exists and hasn't expired
    pub async fn get(&self, key: &K) -> Option<V> {
        let entries = self.entries.read().await;
        
        if let Some(entry) = entries.get(key) {
            if !entry.is_expired(self.ttl) {
                return Some(entry.value.clone());
            }
        }
        
        None
    }

    /// Insert a value into the cache
    /// If the cache is at capacity, removes the oldest entry
    pub async fn insert(&self, key: K, value: V) {
        let mut entries = self.entries.write().await;
        
        // If at capacity, remove oldest entry
        if entries.len() >= self.max_capacity && !entries.contains_key(&key) {
            if let Some(oldest_key) = self.find_oldest_key(&entries) {
                entries.remove(&oldest_key);
            }
        }
        
        entries.insert(key, CacheEntry::new(value));
    }

    /// Remove a specific key from the cache
    pub async fn remove(&self, key: &K) {
        let mut entries = self.entries.write().await;
        entries.remove(key);
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// Remove all expired entries from the cache
    pub async fn evict_expired(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| !entry.is_expired(self.ttl));
    }

    /// Get the number of entries in the cache (including expired ones)
    pub async fn len(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// Check if the cache is empty
    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.read().await;
        entries.is_empty()
    }

    /// Find the oldest entry key for eviction
    fn find_oldest_key(&self, entries: &HashMap<K, CacheEntry<V>>) -> Option<K> {
        entries
            .iter()
            .min_by_key(|(_, entry)| entry.inserted_at)
            .map(|(key, _)| key.clone())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let total = entries.len();
        let expired = entries.values().filter(|e| e.is_expired(self.ttl)).count();
        
        CacheStats {
            total_entries: total,
            expired_entries: expired,
            valid_entries: total - expired,
            capacity: self.max_capacity,
            ttl_secs: self.ttl.as_secs(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub valid_entries: usize,
    pub capacity: usize,
    pub ttl_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = TtlCache::new(Duration::from_secs(60), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = TtlCache::new(Duration::from_millis(100), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        
        // Value should be available immediately
        assert!(cache.get(&"key1".to_string()).await.is_some());
        
        // Wait for expiration
        sleep(Duration::from_millis(150)).await;
        
        // Value should be expired
        assert!(cache.get(&"key1".to_string()).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_capacity() {
        let cache = TtlCache::new(Duration::from_secs(60), 3);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;
        
        assert_eq!(cache.len().await, 3);
        
        // Adding a 4th item should evict the oldest
        cache.insert("key4".to_string(), "value4".to_string()).await;
        
        assert_eq!(cache.len().await, 3);
        assert!(cache.get(&"key1".to_string()).await.is_none());
        assert!(cache.get(&"key4".to_string()).await.is_some());
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = TtlCache::new(Duration::from_secs(60), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        
        assert_eq!(cache.len().await, 2);
        
        cache.clear().await;
        
        assert_eq!(cache.len().await, 0);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_cache_remove() {
        let cache = TtlCache::new(Duration::from_secs(60), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        
        cache.remove(&"key1".to_string()).await;
        
        assert!(cache.get(&"key1".to_string()).await.is_none());
        assert!(cache.get(&"key2".to_string()).await.is_some());
    }

    #[tokio::test]
    async fn test_cache_evict_expired() {
        let cache = TtlCache::new(Duration::from_millis(100), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        
        // Wait for expiration
        sleep(Duration::from_millis(150)).await;
        
        // Add a new entry that won't be expired
        cache.insert("key3".to_string(), "value3".to_string()).await;
        
        assert_eq!(cache.len().await, 3);
        
        // Evict expired entries
        cache.evict_expired().await;
        
        assert_eq!(cache.len().await, 1);
        assert!(cache.get(&"key3".to_string()).await.is_some());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = TtlCache::new(Duration::from_millis(100), 10);
        
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        
        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.valid_entries, 2);
        assert_eq!(stats.expired_entries, 0);
        
        // Wait for expiration
        sleep(Duration::from_millis(150)).await;
        
        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.expired_entries, 2);
        assert_eq!(stats.valid_entries, 0);
    }
}
