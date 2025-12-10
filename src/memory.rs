/// Memory optimization utilities for the MySQL MCP Server
/// 
/// This module provides utilities for monitoring and optimizing memory usage,
/// including memory estimation and resource tracking.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Memory statistics for the server
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryStats {
    /// Estimated memory used by connection pools (bytes)
    pub pool_memory_bytes: usize,
    /// Estimated memory used by caches (bytes)
    pub cache_memory_bytes: usize,
    /// Estimated memory used by active streams (bytes)
    pub stream_memory_bytes: usize,
    /// Total estimated memory usage (bytes)
    pub total_memory_bytes: usize,
}

/// Memory tracker for monitoring resource usage
#[derive(Debug, Clone)]
pub struct MemoryTracker {
    pool_memory: Arc<AtomicUsize>,
    cache_memory: Arc<AtomicUsize>,
    stream_memory: Arc<AtomicUsize>,
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        Self {
            pool_memory: Arc::new(AtomicUsize::new(0)),
            cache_memory: Arc::new(AtomicUsize::new(0)),
            stream_memory: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Add pool memory usage
    pub fn add_pool_memory(&self, bytes: usize) {
        self.pool_memory.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Remove pool memory usage
    pub fn remove_pool_memory(&self, bytes: usize) {
        self.pool_memory.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Add cache memory usage
    pub fn add_cache_memory(&self, bytes: usize) {
        self.cache_memory.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Remove cache memory usage
    pub fn remove_cache_memory(&self, bytes: usize) {
        self.cache_memory.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Add stream memory usage
    pub fn add_stream_memory(&self, bytes: usize) {
        self.stream_memory.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Remove stream memory usage
    pub fn remove_stream_memory(&self, bytes: usize) {
        self.stream_memory.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Get current memory statistics
    pub fn stats(&self) -> MemoryStats {
        let pool_memory_bytes = self.pool_memory.load(Ordering::Relaxed);
        let cache_memory_bytes = self.cache_memory.load(Ordering::Relaxed);
        let stream_memory_bytes = self.stream_memory.load(Ordering::Relaxed);

        MemoryStats {
            pool_memory_bytes,
            cache_memory_bytes,
            stream_memory_bytes,
            total_memory_bytes: pool_memory_bytes + cache_memory_bytes + stream_memory_bytes,
        }
    }

    /// Reset all memory counters
    pub fn reset(&self) {
        self.pool_memory.store(0, Ordering::Relaxed);
        self.cache_memory.store(0, Ordering::Relaxed);
        self.stream_memory.store(0, Ordering::Relaxed);
    }
}

/// Estimate memory usage for a connection pool
pub fn estimate_pool_memory(max_connections: u32) -> usize {
    // Each MySQL connection uses approximately 1.5 MB
    const BYTES_PER_CONNECTION: usize = 1_500_000;
    max_connections as usize * BYTES_PER_CONNECTION
}

/// Estimate memory usage for a cache
pub fn estimate_cache_memory(capacity: usize, avg_entry_size: usize) -> usize {
    capacity * avg_entry_size
}

/// Estimate memory usage for a stream buffer
pub fn estimate_stream_memory(chunk_size: usize, avg_row_size: usize) -> usize {
    chunk_size * avg_row_size
}

/// Calculate total estimated memory for server configuration
pub fn estimate_total_memory(
    num_datasources: usize,
    max_connections_per_datasource: u32,
    cache_capacity: usize,
    avg_cache_entry_size: usize,
    max_concurrent_streams: usize,
    stream_chunk_size: usize,
    avg_row_size: usize,
) -> usize {
    const BASE_OVERHEAD: usize = 75_000_000; // 75 MB base overhead

    let pool_memory = num_datasources * estimate_pool_memory(max_connections_per_datasource);
    let cache_memory = estimate_cache_memory(cache_capacity, avg_cache_entry_size);
    let stream_memory = max_concurrent_streams * estimate_stream_memory(stream_chunk_size, avg_row_size);

    pool_memory + cache_memory + stream_memory + BASE_OVERHEAD
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();

        tracker.add_pool_memory(1000);
        tracker.add_cache_memory(500);
        tracker.add_stream_memory(250);

        let stats = tracker.stats();
        assert_eq!(stats.pool_memory_bytes, 1000);
        assert_eq!(stats.cache_memory_bytes, 500);
        assert_eq!(stats.stream_memory_bytes, 250);
        assert_eq!(stats.total_memory_bytes, 1750);

        tracker.remove_pool_memory(500);
        let stats = tracker.stats();
        assert_eq!(stats.pool_memory_bytes, 500);
        assert_eq!(stats.total_memory_bytes, 1250);

        tracker.reset();
        let stats = tracker.stats();
        assert_eq!(stats.total_memory_bytes, 0);
    }

    #[test]
    fn test_estimate_pool_memory() {
        let memory = estimate_pool_memory(10);
        assert_eq!(memory, 15_000_000); // 10 * 1.5 MB
    }

    #[test]
    fn test_estimate_cache_memory() {
        let memory = estimate_cache_memory(100, 1000);
        assert_eq!(memory, 100_000); // 100 * 1000 bytes
    }

    #[test]
    fn test_estimate_stream_memory() {
        let memory = estimate_stream_memory(1000, 500);
        assert_eq!(memory, 500_000); // 1000 * 500 bytes
    }

    #[test]
    fn test_estimate_total_memory() {
        let total = estimate_total_memory(
            2,      // 2 data sources
            10,     // 10 connections each
            100,    // 100 cache entries
            1000,   // 1 KB per entry
            5,      // 5 concurrent streams
            1000,   // 1000 rows per chunk
            500,    // 500 bytes per row
        );

        // Expected:
        // Pool: 2 * 10 * 1.5 MB = 30 MB
        // Cache: 100 * 1 KB = 100 KB
        // Stream: 5 * 1000 * 500 = 2.5 MB
        // Base: 75 MB
        // Total: ~107.6 MB

        assert!(total > 100_000_000); // > 100 MB
        assert!(total < 120_000_000); // < 120 MB
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1536 * 1024 * 1024), "1.50 GB");
    }
}
