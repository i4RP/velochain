//! In-memory LRU cache layer for frequently accessed chain data.
//!
//! Sits between the application and RocksDB to reduce disk I/O
//! for hot data like recent block headers and account state.

use parking_lot::RwLock;
use std::collections::HashMap;
use velochain_primitives::BlockHeader;

/// Default cache capacity for block headers.
const DEFAULT_HEADER_CACHE_SIZE: usize = 1024;

/// Default cache capacity for block hashes (number -> hash).
const DEFAULT_HASH_CACHE_SIZE: usize = 1024;

/// Simple bounded cache with LRU-style eviction.
///
/// Uses insertion order for eviction when the cache is full.
/// Not a true LRU (doesn't track access order) but sufficient
/// for chain data where recent blocks are most frequently accessed.
struct BoundedCache<K: std::hash::Hash + Eq + Clone, V: Clone> {
    map: HashMap<K, V>,
    order: Vec<K>,
    capacity: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> BoundedCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    fn insert(&mut self, key: K, value: V) {
        if let std::collections::hash_map::Entry::Occupied(mut e) = self.map.entry(key.clone()) {
            e.insert(value);
            return;
        }
        if self.order.len() >= self.capacity {
            // Evict oldest entry
            if let Some(old_key) = self.order.first().cloned() {
                self.map.remove(&old_key);
                self.order.remove(0);
            }
        }
        self.order.push(key.clone());
        self.map.insert(key, value);
    }

    fn remove(&mut self, key: &K) {
        self.map.remove(key);
        self.order.retain(|k| k != key);
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

/// Thread-safe cache for chain data.
pub struct ChainCache {
    /// Cached block headers: block_hash (32 bytes) -> BlockHeader.
    headers: RwLock<BoundedCache<[u8; 32], BlockHeader>>,
    /// Cached block number -> block hash mapping.
    block_hashes: RwLock<BoundedCache<u64, [u8; 32]>>,
    /// Cache hit/miss counters for monitoring.
    stats: RwLock<CacheStats>,
}

/// Cache performance statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub header_hits: u64,
    pub header_misses: u64,
    pub hash_hits: u64,
    pub hash_misses: u64,
}

impl CacheStats {
    /// Header cache hit rate as a percentage.
    pub fn header_hit_rate(&self) -> f64 {
        let total = self.header_hits + self.header_misses;
        if total == 0 {
            return 0.0;
        }
        (self.header_hits as f64 / total as f64) * 100.0
    }

    /// Block hash cache hit rate as a percentage.
    pub fn hash_hit_rate(&self) -> f64 {
        let total = self.hash_hits + self.hash_misses;
        if total == 0 {
            return 0.0;
        }
        (self.hash_hits as f64 / total as f64) * 100.0
    }
}

impl Default for ChainCache {
    fn default() -> Self {
        Self::new(DEFAULT_HEADER_CACHE_SIZE, DEFAULT_HASH_CACHE_SIZE)
    }
}

impl ChainCache {
    /// Create a new chain cache with specified capacities.
    pub fn new(header_capacity: usize, hash_capacity: usize) -> Self {
        Self {
            headers: RwLock::new(BoundedCache::new(header_capacity)),
            block_hashes: RwLock::new(BoundedCache::new(hash_capacity)),
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Get a cached block header.
    pub fn get_header(&self, hash: &[u8; 32]) -> Option<BlockHeader> {
        let cache = self.headers.read();
        match cache.get(hash) {
            Some(header) => {
                self.stats.write().header_hits += 1;
                Some(header.clone())
            }
            None => {
                self.stats.write().header_misses += 1;
                None
            }
        }
    }

    /// Insert a block header into the cache.
    pub fn put_header(&self, hash: [u8; 32], header: BlockHeader) {
        self.headers.write().insert(hash, header);
    }

    /// Remove a header from the cache.
    pub fn remove_header(&self, hash: &[u8; 32]) {
        self.headers.write().remove(hash);
    }

    /// Get a cached block hash by number.
    pub fn get_block_hash(&self, number: u64) -> Option<[u8; 32]> {
        let cache = self.block_hashes.read();
        match cache.get(&number) {
            Some(hash) => {
                self.stats.write().hash_hits += 1;
                Some(*hash)
            }
            None => {
                self.stats.write().hash_misses += 1;
                None
            }
        }
    }

    /// Insert a block number -> hash mapping into the cache.
    pub fn put_block_hash(&self, number: u64, hash: [u8; 32]) {
        self.block_hashes.write().insert(number, hash);
    }

    /// Get current cache statistics.
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// Get the number of cached headers.
    pub fn header_count(&self) -> usize {
        self.headers.read().len()
    }

    /// Get the number of cached block hashes.
    pub fn hash_count(&self) -> usize {
        self.block_hashes.read().len()
    }

    /// Clear all caches.
    pub fn clear(&self) {
        self.headers.write().clear();
        self.block_hashes.write().clear();
        *self.stats.write() = CacheStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velochain_primitives::BlockHeader;

    fn test_header(number: u64) -> BlockHeader {
        let mut h = BlockHeader::genesis(Default::default());
        h.number = number;
        h.timestamp = 1000 + number;
        h
    }

    #[test]
    fn test_cache_put_get_header() {
        let cache = ChainCache::default();
        let header = test_header(1);
        let hash = [0xAA; 32];
        cache.put_header(hash, header.clone());
        let retrieved = cache.get_header(&hash).unwrap();
        assert_eq!(retrieved.number, 1);
    }

    #[test]
    fn test_cache_miss() {
        let cache = ChainCache::default();
        assert!(cache.get_header(&[0x00; 32]).is_none());
    }

    #[test]
    fn test_cache_put_get_block_hash() {
        let cache = ChainCache::default();
        let hash = [0xBB; 32];
        cache.put_block_hash(5, hash);
        assert_eq!(cache.get_block_hash(5), Some(hash));
        assert_eq!(cache.get_block_hash(6), None);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = ChainCache::new(2, 2);
        let h1 = test_header(1);
        let h2 = test_header(2);
        let h3 = test_header(3);

        cache.put_header([0x01; 32], h1);
        cache.put_header([0x02; 32], h2);
        assert_eq!(cache.header_count(), 2);

        // This should evict the first entry
        cache.put_header([0x03; 32], h3);
        assert_eq!(cache.header_count(), 2);
        assert!(cache.get_header(&[0x01; 32]).is_none()); // evicted
        assert!(cache.get_header(&[0x03; 32]).is_some()); // present
    }

    #[test]
    fn test_cache_stats() {
        let cache = ChainCache::default();
        let header = test_header(1);
        cache.put_header([0xAA; 32], header);

        cache.get_header(&[0xAA; 32]); // hit
        cache.get_header(&[0xBB; 32]); // miss

        let stats = cache.stats();
        assert_eq!(stats.header_hits, 1);
        assert_eq!(stats.header_misses, 1);
        assert!((stats.header_hit_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_cache_clear() {
        let cache = ChainCache::default();
        cache.put_header([0xAA; 32], test_header(1));
        cache.put_block_hash(1, [0xAA; 32]);
        assert_eq!(cache.header_count(), 1);
        assert_eq!(cache.hash_count(), 1);

        cache.clear();
        assert_eq!(cache.header_count(), 0);
        assert_eq!(cache.hash_count(), 0);
    }

    #[test]
    fn test_cache_update_existing() {
        let cache = ChainCache::default();
        let h1 = test_header(1);
        let mut h2 = test_header(2);
        h2.number = 1; // same hash key, different content

        cache.put_header([0xAA; 32], h1);
        cache.put_header([0xAA; 32], h2);
        assert_eq!(cache.header_count(), 1); // should not grow
    }

    #[test]
    fn test_remove_header() {
        let cache = ChainCache::default();
        cache.put_header([0xAA; 32], test_header(1));
        assert!(cache.get_header(&[0xAA; 32]).is_some());
        cache.remove_header(&[0xAA; 32]);
        assert!(cache.get_header(&[0xAA; 32]).is_none());
    }
}
