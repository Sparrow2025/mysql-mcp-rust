# Performance Optimization Guide

This document describes the performance optimizations implemented in the MySQL MCP Server and provides guidance on tuning for different workloads.

## Overview

The MySQL MCP Server has been optimized for:
- **High throughput**: Handle many concurrent queries efficiently
- **Low latency**: Minimize response times for individual queries
- **Resource efficiency**: Optimize memory and connection usage
- **Scalability**: Support multiple data sources and databases

## Key Optimizations

### 1. Connection Pool Optimization

#### Default Configuration
```toml
[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

#### Optimized for High Throughput
```toml
[data_sources.pool_config]
max_connections = 20          # Increased for better concurrency
min_connections = 5           # Higher minimum reduces cold-start latency
connection_timeout_secs = 15  # Fail faster on connection issues
idle_timeout_secs = 180       # Reclaim idle connections faster
max_lifetime_secs = 1200      # Prevent stale connections
```

#### Tuning Guidelines

**max_connections**:
- Set to 2-3x expected concurrent queries
- Too low: Queries queue, increasing latency
- Too high: Database server overhead, diminishing returns
- Monitor: Active connection count vs. max_connections

**min_connections**:
- Set to ~25% of max_connections
- Keeps pool warm, reduces connection establishment overhead
- Trade-off: Idle connections consume resources

**connection_timeout_secs**:
- Lower values (10-20s): Fail-fast behavior, better for high-availability
- Higher values (30-60s): More tolerant of network issues
- Consider: Network latency to database server

**idle_timeout_secs**:
- Lower values (120-240s): Reclaim resources faster
- Higher values (300-600s): Reduce connection churn
- Consider: Query frequency and pattern

**max_lifetime_secs**:
- Lower values (900-1500s): Prevent stale connections, better for long-running servers
- Higher values (1800-3600s): Reduce connection churn
- Consider: Database server connection limits

### 2. Caching Strategy

The server implements a TTL-based cache for expensive operations:

#### Database List Caching
- **Default TTL**: 60 seconds
- **Purpose**: Reduce repeated `SHOW DATABASES` queries
- **Trade-off**: Stale data vs. reduced database load

#### Schema Information Caching
- **Default TTL**: 300 seconds (5 minutes)
- **Purpose**: Reduce repeated schema queries
- **Trade-off**: Stale schema vs. reduced database load

#### Cache Configuration

```rust
// In your application code
use mysql_mcp_server::cache::TtlCache;
use std::time::Duration;

// Create a cache with 60-second TTL and 100-entry capacity
let cache = TtlCache::new(Duration::from_secs(60), 100);
```

#### Cache Tuning Guidelines

**TTL (Time To Live)**:
- Shorter TTL (30-60s): More up-to-date data, higher database load
- Longer TTL (300-600s): Lower database load, potentially stale data
- Consider: How frequently schema changes occur

**Capacity**:
- Smaller capacity (50-100): Lower memory usage, more evictions
- Larger capacity (500-1000): Higher memory usage, fewer evictions
- Consider: Number of unique databases/tables accessed

### 3. Stream Chunk Size Optimization

#### Default Configuration
```toml
stream_chunk_size = 1000
```

#### Optimized Configuration
```toml
stream_chunk_size = 2000  # Increased for better throughput
```

#### Tuning Guidelines

**Smaller chunks (500-1000 rows)**:
- Lower memory usage per query
- Better for low-bandwidth networks
- More responsive for interactive queries
- Higher overhead (more round-trips)

**Larger chunks (2000-5000 rows)**:
- Higher throughput for large result sets
- Better for high-bandwidth networks
- Lower overhead (fewer round-trips)
- Higher memory usage per query

**Considerations**:
- Network bandwidth and latency
- Client memory constraints
- Typical result set sizes
- Interactive vs. batch workloads

### 4. Query Timeout Optimization

#### Default Configuration
```toml
query_timeout_secs = 30
```

#### Tuning Guidelines

**Shorter timeout (10-20s)**:
- Prevents runaway queries from consuming resources
- Better for OLTP workloads with predictable query times
- May timeout legitimate complex queries

**Longer timeout (60-120s)**:
- Allows complex analytical queries to complete
- Better for OLAP workloads
- Risk of resource exhaustion from slow queries

**Recommendations**:
- OLTP workloads: 15-30 seconds
- OLAP workloads: 60-300 seconds
- Mixed workloads: 30-60 seconds with separate data sources

### 5. Memory Optimization

#### Connection Pool Memory
- Each connection consumes ~1-2 MB of memory
- Formula: `memory_per_datasource ≈ max_connections × 1.5 MB`
- Example: 20 connections ≈ 30 MB per data source

#### Cache Memory
- Each cache entry size depends on data
- Database list: ~100 bytes per database
- Schema info: ~1-10 KB per table
- Formula: `cache_memory ≈ capacity × avg_entry_size`

#### Stream Buffer Memory
- Memory per active stream: `chunk_size × row_size`
- Example: 2000 rows × 1 KB/row = 2 MB per stream
- Consider: Maximum concurrent streams

#### Total Memory Estimate
```
Total Memory ≈ 
  (num_datasources × max_connections × 1.5 MB) +
  (cache_capacity × avg_entry_size) +
  (max_concurrent_streams × chunk_size × avg_row_size) +
  base_overhead (50-100 MB)
```

## Performance Benchmarking

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench cache

# Save baseline for comparison
cargo bench --bench performance_benchmark -- --save-baseline main

# Compare against baseline
cargo bench --bench performance_benchmark -- --baseline main
```

### Benchmark Results Interpretation

**Cache Operations**:
- Insert: Should be < 1 µs per operation
- Get: Should be < 500 ns per operation
- Evict: Should be < 100 µs for 100 entries

**Config Parsing**:
- TOML parsing: Should be < 100 µs for typical config

**Manager Operations**:
- Create: Should be < 1 ms for 10 data sources
- Get source: Should be < 100 ns per operation
- List sources: Should be < 10 µs for 10 data sources
- Validate key: Should be < 100 ns per operation

## Workload-Specific Recommendations

### High-Throughput OLTP

```toml
query_timeout_secs = 15
stream_chunk_size = 1000

[data_sources.pool_config]
max_connections = 30
min_connections = 10
connection_timeout_secs = 10
idle_timeout_secs = 120
max_lifetime_secs = 900
```

**Rationale**:
- High connection count for concurrency
- Short timeouts for fail-fast behavior
- Smaller chunks for lower latency
- Aggressive connection recycling

### Analytical/OLAP Workloads

```toml
query_timeout_secs = 120
stream_chunk_size = 5000

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 600
max_lifetime_secs = 3600
```

**Rationale**:
- Moderate connection count (queries are long-running)
- Long timeouts for complex queries
- Large chunks for high throughput
- Longer connection lifetimes

### Mixed Workloads

```toml
query_timeout_secs = 30
stream_chunk_size = 2000

[data_sources.pool_config]
max_connections = 20
min_connections = 5
connection_timeout_secs = 15
idle_timeout_secs = 180
max_lifetime_secs = 1200
```

**Rationale**:
- Balanced settings for diverse query patterns
- Moderate timeouts accommodate most queries
- Medium chunks balance latency and throughput

### Low-Traffic/Development

```toml
query_timeout_secs = 30
stream_chunk_size = 1000

[data_sources.pool_config]
max_connections = 5
min_connections = 1
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

**Rationale**:
- Minimal resource usage
- Default timeouts sufficient
- Standard chunk size
- Conservative connection management

## Monitoring and Tuning

### Key Metrics to Monitor

1. **Connection Pool Utilization**
   - Active connections / max_connections
   - Target: 50-80% during peak load
   - Action: Increase max_connections if consistently > 90%

2. **Query Latency**
   - P50, P95, P99 query response times
   - Target: Depends on workload (OLTP: < 100ms, OLAP: < 10s)
   - Action: Investigate slow queries, optimize indexes

3. **Timeout Rate**
   - Percentage of queries timing out
   - Target: < 1%
   - Action: Increase timeout or optimize queries

4. **Cache Hit Rate**
   - Cache hits / (cache hits + cache misses)
   - Target: > 80% for frequently accessed data
   - Action: Increase cache capacity or TTL

5. **Memory Usage**
   - Total server memory consumption
   - Target: < 80% of available memory
   - Action: Reduce pool sizes, cache capacity, or chunk size

### Tuning Process

1. **Establish Baseline**
   - Run benchmarks with default configuration
   - Monitor metrics under typical load
   - Document baseline performance

2. **Identify Bottlenecks**
   - High connection pool utilization → Increase max_connections
   - High query latency → Optimize queries or increase timeout
   - High timeout rate → Increase timeout or optimize queries
   - Low cache hit rate → Increase cache capacity or TTL
   - High memory usage → Reduce pool sizes or chunk size

3. **Make Incremental Changes**
   - Change one parameter at a time
   - Test under load
   - Measure impact on metrics
   - Keep or revert change

4. **Iterate**
   - Repeat until performance targets are met
   - Document final configuration
   - Monitor for regressions

## Best Practices

1. **Start Conservative**: Begin with default settings and tune based on observed behavior
2. **Monitor Continuously**: Track key metrics to detect performance degradation
3. **Test Under Load**: Performance characteristics change under load
4. **Document Changes**: Keep a log of configuration changes and their impact
5. **Use Separate Data Sources**: Different workloads may need different settings
6. **Plan for Growth**: Leave headroom for traffic increases
7. **Regular Review**: Revisit configuration as workload patterns evolve

## Troubleshooting

### High Latency

**Symptoms**: Queries taking longer than expected

**Possible Causes**:
- Connection pool exhausted (queries queuing)
- Network latency to database
- Slow queries or missing indexes
- Database server overloaded

**Solutions**:
- Increase max_connections
- Optimize queries
- Add database indexes
- Scale database server

### High Memory Usage

**Symptoms**: Server consuming excessive memory

**Possible Causes**:
- Too many connections
- Large cache capacity
- Large stream chunk size
- Memory leaks

**Solutions**:
- Reduce max_connections
- Reduce cache capacity
- Reduce stream_chunk_size
- Check for memory leaks

### Connection Errors

**Symptoms**: Frequent connection failures

**Possible Causes**:
- Database server unreachable
- Connection timeout too short
- Database connection limit reached
- Network issues

**Solutions**:
- Verify database connectivity
- Increase connection_timeout_secs
- Reduce max_connections
- Check database server logs

### Stale Data

**Symptoms**: Cached data not reflecting recent changes

**Possible Causes**:
- Cache TTL too long
- Cache not being invalidated

**Solutions**:
- Reduce cache TTL
- Implement cache invalidation on schema changes
- Disable caching for frequently changing data

## Conclusion

Performance optimization is an iterative process that requires understanding your workload, monitoring key metrics, and making informed configuration changes. Start with the recommended defaults, monitor performance, and tune based on observed behavior. The optimizations described in this guide provide a solid foundation for achieving excellent performance across a wide range of workloads.
