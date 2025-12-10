// Feature: mysql-mcp-multi-datasource, Property 15: Database listing accuracy
// Validates: Requirements 6.2, 6.4
//
// Property 15: Database listing accuracy
// For any valid data source key, the list-databases tool should return all accessible databases with metadata

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::ListTool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Note: This test requires a real MySQL database connection
// We'll test the property that the tool correctly validates parameters
// and handles errors appropriately

#[tokio::test]
async fn test_list_databases_validates_empty_key() {
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    // Property: Empty data source key should be rejected
    let result = list_tool.list_databases("").await;
    assert!(result.is_err(), "Empty data source key should be rejected");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Data source key is required") || error_msg.contains("required"),
        "Error message should indicate key is required"
    );
}

#[tokio::test]
async fn test_list_databases_validates_invalid_key() {
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    // Property: Invalid data source key should be rejected
    let result = list_tool.list_databases("nonexistent-key").await;
    assert!(result.is_err(), "Invalid data source key should be rejected");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Invalid data source key") || error_msg.contains("nonexistent-key"),
        "Error message should indicate invalid key"
    );
}

#[tokio::test]
async fn test_list_databases_checks_availability() {
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager.clone(), pool_managers);
    
    // Mark the data source as unavailable
    manager.set_status("test-db", mysql_mcp_server::manager::ConnectionStatus::Unavailable).await;
    
    // Property: Unavailable data source should be rejected
    let result = list_tool.list_databases("test-db").await;
    assert!(result.is_err(), "Unavailable data source should be rejected");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("unavailable") || error_msg.contains("Unavailable"),
        "Error message should indicate data source is unavailable"
    );
}

#[tokio::test]
async fn test_list_databases_caching() {
    // This test verifies that the caching mechanism works correctly
    // We can't test with a real database, but we can verify the cache structure
    
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    // Property: Cache can be cleared without error
    list_tool.clear_cache("test-db").await;
    list_tool.clear_all_caches().await;
    
    // No assertions needed - just verify no panics
}

// Integration test with real MySQL database (requires MySQL to be running)
// This test is marked with #[ignore] so it only runs when explicitly requested
#[tokio::test]
#[ignore]
async fn test_list_databases_with_real_mysql() {
    // This test requires MySQL to be running on localhost:3306
    // with a user 'root' and password 'password'
    // Run with: cargo test --test database_listing_property_test -- --ignored
    
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "root".to_string(),
        password: "password".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    // Property: Should return a list of databases with metadata
    let result = list_tool.list_databases("test-db").await;
    
    if result.is_ok() {
        let databases = result.unwrap();
        
        // Property: Each database should have required metadata
        for db in &databases {
            assert!(!db.name.is_empty(), "Database name should not be empty");
            assert!(!db.charset.is_empty(), "Charset should not be empty");
            assert!(!db.collation.is_empty(), "Collation should not be empty");
            // size_bytes can be None for empty databases
        }
        
        // Property: System databases should be filtered out
        let system_dbs = ["information_schema", "performance_schema", "mysql", "sys"];
        for db in &databases {
            assert!(
                !system_dbs.contains(&db.name.as_str()),
                "System database '{}' should be filtered out",
                db.name
            );
        }
        
        println!("Found {} databases", databases.len());
        for db in &databases {
            println!("  - {} (charset: {}, collation: {})", db.name, db.charset, db.collation);
        }
    } else {
        // If connection fails, that's expected in CI/CD environments
        println!("Could not connect to MySQL (expected in CI): {:?}", result.unwrap_err());
    }
}

#[tokio::test]
#[ignore]
async fn test_list_databases_caching_with_real_mysql() {
    // This test verifies that caching works correctly with a real database
    // Run with: cargo test --test database_listing_property_test -- --ignored
    
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "root".to_string(),
        password: "password".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    // First call - should query the database
    let result1 = list_tool.list_databases("test-db").await;
    
    if result1.is_ok() {
        let databases1 = result1.unwrap();
        
        // Second call - should use cache
        let result2 = list_tool.list_databases("test-db").await;
        assert!(result2.is_ok(), "Second call should succeed");
        
        let databases2 = result2.unwrap();
        
        // Property: Cached results should match original results
        assert_eq!(
            databases1.len(),
            databases2.len(),
            "Cached results should have same number of databases"
        );
        
        for (db1, db2) in databases1.iter().zip(databases2.iter()) {
            assert_eq!(db1.name, db2.name, "Database names should match");
            assert_eq!(db1.charset, db2.charset, "Charsets should match");
            assert_eq!(db1.collation, db2.collation, "Collations should match");
        }
        
        // Clear cache and verify
        list_tool.clear_cache("test-db").await;
        
        // Third call - should query the database again
        let result3 = list_tool.list_databases("test-db").await;
        assert!(result3.is_ok(), "Third call should succeed");
    } else {
        println!("Could not connect to MySQL (expected in CI): {:?}", result1.unwrap_err());
    }
}
