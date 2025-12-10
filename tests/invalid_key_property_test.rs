// Feature: mysql-mcp-multi-datasource, Property 16: Invalid data source key handling
// Validates: Requirements 6.3, 8.4
//
// Property 16: Invalid data source key handling
// For any invalid data source key, operations should return an authentication error

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::error::McpError;
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::{ListTool, QueryTool, ExecuteTool, SchemaTool};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

// Generator for valid data source keys
fn arb_valid_key() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{2,15}"
}

// Generator for invalid keys (keys that don't exist in the configuration)
fn arb_invalid_key() -> impl Strategy<Value = String> {
    prop_oneof![
        // Random strings that are unlikely to match
        "[A-Z][A-Z0-9-]{2,15}",  // Uppercase (valid keys are lowercase)
        "invalid-[0-9]{5}",       // Prefixed with "invalid-"
        "nonexistent-[a-z]{3}",   // Prefixed with "nonexistent-"
        "fake-db-[0-9]{2}",       // Prefixed with "fake-db-"
    ]
}

// Generator for a data source configuration
fn arb_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    arb_valid_key().prop_map(|key| DataSourceConfig {
        key,
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn test_list_databases_rejects_invalid_key(
        config in arb_datasource_config(),
        invalid_key in arb_invalid_key()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Ensure the invalid key is actually different from the valid key
            prop_assume!(invalid_key != config.key);
            
            let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let list_tool = ListTool::new(manager, pool_managers);
            
            // Property: Invalid key should be rejected
            let result = list_tool.list_databases(&invalid_key).await;
            prop_assert!(result.is_err(), "Invalid key should be rejected");
            
            // Property: Error should be InvalidDataSourceKey
            match result.unwrap_err() {
                McpError::InvalidDataSourceKey(key) => {
                    prop_assert_eq!(key, invalid_key, "Error should contain the invalid key");
                }
                other => {
                    return Err(TestCaseError::fail(format!(
                        "Expected InvalidDataSourceKey error, got: {:?}",
                        other
                    )));
                }
            }
            
            Ok(())
        })?;
    }
    
    #[test]
    fn test_query_tool_rejects_invalid_key(
        config in arb_datasource_config(),
        invalid_key in arb_invalid_key()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            prop_assume!(invalid_key != config.key);
            
            let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let query_tool = QueryTool::new(manager, pool_managers);
            
            // Property: Invalid key should be rejected
            let result = query_tool.execute(&invalid_key, "testdb", "SELECT 1").await;
            prop_assert!(result.is_err(), "Invalid key should be rejected");
            
            // Property: Error should be InvalidDataSourceKey
            match result.unwrap_err() {
                McpError::InvalidDataSourceKey(key) => {
                    prop_assert_eq!(key, invalid_key, "Error should contain the invalid key");
                }
                other => {
                    return Err(TestCaseError::fail(format!(
                        "Expected InvalidDataSourceKey error, got: {:?}",
                        other
                    )));
                }
            }
            
            Ok(())
        })?;
    }
    
    #[test]
    fn test_execute_tool_rejects_invalid_key(
        config in arb_datasource_config(),
        invalid_key in arb_invalid_key()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            prop_assume!(invalid_key != config.key);
            
            let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let execute_tool = ExecuteTool::new(manager, pool_managers);
            
            // Property: Invalid key should be rejected
            let result = execute_tool.execute(&invalid_key, "testdb", "INSERT INTO test VALUES (1)").await;
            prop_assert!(result.is_err(), "Invalid key should be rejected");
            
            // Property: Error should be InvalidDataSourceKey
            match result.unwrap_err() {
                McpError::InvalidDataSourceKey(key) => {
                    prop_assert_eq!(key, invalid_key, "Error should contain the invalid key");
                }
                other => {
                    return Err(TestCaseError::fail(format!(
                        "Expected InvalidDataSourceKey error, got: {:?}",
                        other
                    )));
                }
            }
            
            Ok(())
        })?;
    }
    
    #[test]
    fn test_schema_tool_rejects_invalid_key(
        config in arb_datasource_config(),
        invalid_key in arb_invalid_key()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            prop_assume!(invalid_key != config.key);
            
            let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let schema_tool = SchemaTool::new(manager, pool_managers);
            
            // Property: Invalid key should be rejected for list_tables
            let result = schema_tool.list_tables(&invalid_key, "testdb").await;
            prop_assert!(result.is_err(), "Invalid key should be rejected");
            
            match result.unwrap_err() {
                McpError::InvalidDataSourceKey(key) => {
                    prop_assert_eq!(key, invalid_key, "Error should contain the invalid key");
                }
                other => {
                    return Err(TestCaseError::fail(format!(
                        "Expected InvalidDataSourceKey error, got: {:?}",
                        other
                    )));
                }
            }
            
            Ok(())
        })?;
    }
    
    #[test]
    fn test_multiple_invalid_keys_all_rejected(
        configs in proptest::collection::vec(arb_datasource_config(), 1..5),
        invalid_keys in proptest::collection::vec(arb_invalid_key(), 1..10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Make configs have unique keys
            let mut seen_keys = HashSet::new();
            let mut unique_configs = Vec::new();
            
            for mut config in configs {
                let mut key = config.key.clone();
                let mut counter = 0;
                
                while seen_keys.contains(&key) {
                    counter += 1;
                    key = format!("{}-{}", config.key, counter);
                }
                
                config.key = key.clone();
                seen_keys.insert(key);
                unique_configs.push(config);
            }
            
            let valid_keys: HashSet<String> = unique_configs.iter().map(|c| c.key.clone()).collect();
            
            let manager = Arc::new(DataSourceManager::new(unique_configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let list_tool = ListTool::new(manager, pool_managers);
            
            // Property: All invalid keys should be rejected
            for invalid_key in &invalid_keys {
                // Skip if the invalid key happens to match a valid key
                if valid_keys.contains(invalid_key) {
                    continue;
                }
                
                let result = list_tool.list_databases(invalid_key).await;
                prop_assert!(
                    result.is_err(),
                    "Invalid key '{}' should be rejected",
                    invalid_key
                );
                
                match result.unwrap_err() {
                    McpError::InvalidDataSourceKey(_) => {
                        // Expected error type
                    }
                    other => {
                        return Err(TestCaseError::fail(format!(
                            "Expected InvalidDataSourceKey error for '{}', got: {:?}",
                            invalid_key, other
                        )));
                    }
                }
            }
            
            Ok(())
        })?;
    }
}

#[tokio::test]
async fn test_empty_key_is_rejected() {
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
    
    // Test ListTool
    let list_tool = ListTool::new(manager.clone(), pool_managers.clone());
    let result = list_tool.list_databases("").await;
    assert!(result.is_err(), "Empty key should be rejected by ListTool");
    
    // Test QueryTool
    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());
    let result = query_tool.execute("", "testdb", "SELECT 1").await;
    assert!(result.is_err(), "Empty key should be rejected by QueryTool");
    
    // Test ExecuteTool
    let execute_tool = ExecuteTool::new(manager.clone(), pool_managers.clone());
    let result = execute_tool.execute("", "testdb", "INSERT INTO test VALUES (1)").await;
    assert!(result.is_err(), "Empty key should be rejected by ExecuteTool");
    
    // Test SchemaTool
    let schema_tool = SchemaTool::new(manager, pool_managers);
    let result = schema_tool.list_tables("", "testdb").await;
    assert!(result.is_err(), "Empty key should be rejected by SchemaTool");
}

#[tokio::test]
async fn test_valid_key_is_accepted() {
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
    
    // Property: Valid key should pass validation
    let result = manager.validate_key("test-db");
    assert!(result.is_ok(), "Valid key should be accepted");
    
    // Property: Invalid key should fail validation
    let result = manager.validate_key("invalid-key");
    assert!(result.is_err(), "Invalid key should be rejected");
    
    match result.unwrap_err() {
        McpError::InvalidDataSourceKey(key) => {
            assert_eq!(key, "invalid-key", "Error should contain the invalid key");
        }
        other => {
            panic!("Expected InvalidDataSourceKey error, got: {:?}", other);
        }
    }
}
