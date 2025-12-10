// Performance benchmarks for MySQL MCP Server
// 
// Run with: cargo bench
//
// These benchmarks measure:
// - Connection pool performance
// - Cache performance
// - Configuration parsing performance
// - Data source manager operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use mysql_mcp_server::cache::TtlCache;
use mysql_mcp_server::config::{DataSourceConfig, PoolConfig, ServerConfig};
use mysql_mcp_server::manager::DataSourceManager;
use std::time::Duration;

// Benchmark cache operations
fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache");
    
    // Benchmark cache insertion
    group.bench_function("insert", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cache = TtlCache::new(Duration::from_secs(60), 1000);
        
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                cache.insert(
                    black_box(format!("key_{}", i)),
                    black_box(format!("value_{}", i))
                ).await;
            }
        });
    });
    
    // Benchmark cache retrieval
    group.bench_function("get", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cache = TtlCache::new(Duration::from_secs(60), 1000);
        
        // Pre-populate cache
        rt.block_on(async {
            for i in 0..100 {
                cache.insert(format!("key_{}", i), format!("value_{}", i)).await;
            }
        });
        
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let _ = cache.get(&black_box(format!("key_{}", i))).await;
            }
        });
    });
    
    // Benchmark cache eviction
    group.bench_function("evict_expired", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cache = TtlCache::new(Duration::from_millis(1), 1000);
        
        b.to_async(&rt).iter(|| async {
            // Populate with expired entries
            for i in 0..100 {
                cache.insert(format!("key_{}", i), format!("value_{}", i)).await;
            }
            
            // Wait for expiration
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            // Evict
            cache.evict_expired().await;
        });
    });
    
    group.finish();
}

// Benchmark configuration parsing
fn bench_config_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("config");
    
    let toml_content = r#"
        query_timeout_secs = 30
        stream_chunk_size = 1000
        
        [[data_sources]]
        key = "test-db"
        name = "Test Database"
        host = "localhost"
        port = 3306
        username = "user"
        password = "pass"
        databases = []
        
        [data_sources.pool_config]
        max_connections = 10
        min_connections = 2
        connection_timeout_secs = 30
        idle_timeout_secs = 300
        max_lifetime_secs = 1800
    "#;
    
    group.bench_function("parse_toml", |b| {
        b.iter(|| {
            let config: ServerConfig = toml::from_str(black_box(toml_content)).unwrap();
            black_box(config);
        });
    });
    
    group.finish();
}

// Benchmark data source manager operations
fn bench_manager_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("manager");
    
    // Create test configurations
    let configs: Vec<DataSourceConfig> = (0..10)
        .map(|i| DataSourceConfig {
            key: format!("db_{}", i),
            name: format!("Database {}", i),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::Query,
        })
        .collect();
    
    // Benchmark manager creation
    group.bench_function("create", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let manager = DataSourceManager::new(black_box(configs.clone())).await.unwrap();
            black_box(manager);
        });
    });
    
    // Benchmark get_source operation
    group.bench_function("get_source", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let manager = rt.block_on(DataSourceManager::new(configs.clone())).unwrap();
        
        b.iter(|| {
            for i in 0..10 {
                let source = manager.get_source(&black_box(format!("db_{}", i)));
                black_box(source);
            }
        });
    });
    
    // Benchmark list_sources operation
    group.bench_function("list_sources", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let manager = rt.block_on(DataSourceManager::new(configs.clone())).unwrap();
        
        b.to_async(&rt).iter(|| async {
            let sources = manager.list_sources().await;
            black_box(sources);
        });
    });
    
    // Benchmark validate_key operation
    group.bench_function("validate_key", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let manager = rt.block_on(DataSourceManager::new(configs.clone())).unwrap();
        
        b.iter(|| {
            for i in 0..10 {
                let result = manager.validate_key(&black_box(format!("db_{}", i)));
                black_box(result);
            }
        });
    });
    
    group.finish();
}

// Benchmark different cache sizes
fn bench_cache_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_sizes");
    
    for size in [100, 500, 1000, 5000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let cache = TtlCache::new(Duration::from_secs(60), size);
            
            // Pre-populate to capacity
            rt.block_on(async {
                for i in 0..size {
                    cache.insert(format!("key_{}", i), format!("value_{}", i)).await;
                }
            });
            
            b.to_async(&rt).iter(|| async {
                // Mix of reads and writes
                for i in 0..100 {
                    if i % 2 == 0 {
                        let _ = cache.get(&black_box(format!("key_{}", i % size))).await;
                    } else {
                        cache.insert(
                            black_box(format!("key_{}", i % size)),
                            black_box(format!("value_{}", i))
                        ).await;
                    }
                }
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_cache_operations,
    bench_config_parsing,
    bench_manager_operations,
    bench_cache_sizes
);
criterion_main!(benches);
